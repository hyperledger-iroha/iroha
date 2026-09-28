//! Hash and transcript primitives using SHA3-256 via `tiny-keccak`.
use tiny_keccak::{Hasher as _, Sha3};
/// SHA3-256 digest size in bytes.
pub const SHA3_256_SIZE: usize = 32;
/// SHA3-512 digest size in bytes.
pub const SHA3_512_SIZE: usize = 64;
/// Computes a SHA3-256 digest of the provided data.
pub fn sha3_256(data: &[u8]) -> [u8; SHA3_256_SIZE] {
    let mut hasher = Sha3::v256();
    hasher.update(data);
    let mut out = [0u8; SHA3_256_SIZE];
    hasher.finalize(&mut out);
    out
}
/// Computes a SHA3-512 digest of the provided data.
pub fn sha3_512(data: &[u8]) -> [u8; SHA3_512_SIZE] {
    let mut hasher = Sha3::v512();
    hasher.update(data);
    let mut out = [0u8; SHA3_512_SIZE];
    hasher.finalize(&mut out);
    out
}
