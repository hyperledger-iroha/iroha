//! Shared test-only ABI-v1 golden authenticated by the owning IVM ABI surface.

/// Exact existing first-release ABI-v1 golden; no production ABI authority is defined here.
pub const ABI_V1_HASH_GOLDEN: &str =
    "696ea3f7d04bfd6585a90f76b288da3de3caa5c4cf4f256949486b1890ce40bd";

/// Decode the exact wire-fixture hash into fixed stack storage for Model codec tests.
/// The owning `ivm_abi` test recomputes and authenticates this value.
pub fn abi_v1_hash() -> [u8; 32] {
    let mut hash = [0; 32];
    hex::decode_to_slice(ABI_V1_HASH_GOLDEN, &mut hash).expect("canonical ABI-v1 golden hash");
    hash
}
