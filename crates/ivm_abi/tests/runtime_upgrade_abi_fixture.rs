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

/// ABI descriptor hash of C10's own ABI V1 surface (`IvmProfileId::C10_ABI_V1`, the host profile
/// of the `iroha.runtime.ivm_c10.v1` release addition). `specs/first_release_history_cutover.json`
/// declares it in `declared_profile_abi_hashes`, with the C10 fixtures that carry it.
const C10_ABI_V1_HASH_GOLDEN: &str =
    "bdd5188fba28f664a4c672746934450ef1aa4a2e53fe3f3a5433ea0ff73dd849";

#[test]
fn c10_abi_v1_golden_matches_the_owning_surface() {
    let current = ivm_abi::IvmProfileId::C10_ABI_V1.abi_hash();
    assert_eq!(
        current,
        ivm_abi::syscalls::compute_c10_abi_hash(SyscallPolicy::AbiV1)
    );
    assert_eq!(hex::encode(current), C10_ABI_V1_HASH_GOLDEN);
    assert_ne!(current, compute_abi_hash(SyscallPolicy::AbiV1));
}
