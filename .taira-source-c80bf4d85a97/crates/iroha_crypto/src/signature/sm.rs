#![allow(dead_code)]
#![allow(clippy::missing_panics_doc)]

use sm2::dsa::Signature as Sm2Signature;
use sm3::digest::{Digest, Output};
use sm4::cipher::{Block, BlockEncrypt, KeyInit};

/// Exercise SM3 hashing so optional dependencies build during the spike.
pub(crate) fn self_test_digest() -> Output<sm3::Sm3> {
    let mut hasher = sm3::Sm3::new();
    hasher.update(b"Iroha");
    hasher.finalize()
}

/// Exercise SM4 block encryption to confirm cipher traits link correctly.
pub(crate) fn self_test_block() -> [u8; 16] {
    let key = [0u8; 16];
    let cipher = sm4::Sm4::new((&key).into());
    let mut block = Block::<sm4::Sm4>::default();
    cipher.encrypt_block(&mut block);
    let mut out = [0u8; 16];
    out.copy_from_slice(block.as_ref());
    out
}

/// Parse a canonical SM2 signature (r∥s) and surface the underlying error type.
pub(crate) fn parse_signature(bytes: &[u8; 64]) -> Result<Sm2Signature, signature::Error> {
    Sm2Signature::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_test_block_matches_sm4_zero_key_zero_block_vector() {
        assert_eq!(
            self_test_block(),
            [
                0x9f, 0x1f, 0x7b, 0xff, 0x6f, 0x55, 0x11, 0x38, 0x4d, 0x94, 0x30, 0x53, 0x1e, 0x53,
                0x8f, 0xd3,
            ]
        );
    }
}
