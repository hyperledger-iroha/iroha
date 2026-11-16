//! Shared test-only ABI-v1 golden authenticated by the owning IVM ABI surface.

/// Exact existing first-release ABI-v1 golden; no production ABI authority is defined here.
pub const ABI_V1_HASH_GOLDEN: &str =
    "7531b45dd9d79aabc3a0f64ab30a053af847dfe248078cff839ab2e14e3261a7";

/// Decode the exact wire-fixture hash into fixed stack storage for Model codec tests.
/// The owning `ivm_abi` test recomputes and authenticates this value.
pub fn abi_v1_hash() -> [u8; 32] {
    let mut hash = [0; 32];
    hex::decode_to_slice(ABI_V1_HASH_GOLDEN, &mut hash).expect("canonical ABI-v1 golden hash");
    hash
}
