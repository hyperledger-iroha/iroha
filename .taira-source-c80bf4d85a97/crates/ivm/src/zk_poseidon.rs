//! Bridge for Poseidon gadget selection.
//!
//! These helpers delegate to the canonical gadget exposed in
//! `iroha_zkp_halo2::poseidon` so that both the VM and Halo2 circuits rely on
//! the same permutation parameters.

/// Hash a pair of `u64` values using Poseidon2 (width 3) and return the low 64 bits.
#[inline]
pub fn pair_hash_u64(a: u64, b: u64) -> u64 {
    iroha_zkp_halo2::poseidon::hash2_u64(a, b)
}

/// Hash a pair of `u64` values and return the canonical 32-byte field encoding.
#[inline]
pub fn pair_hash_bytes(a: u64, b: u64) -> [u8; 32] {
    iroha_zkp_halo2::poseidon::hash2_bytes(a, b)
}
