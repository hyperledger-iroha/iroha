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
    let cipher = sm4::Sm4::new_from_slice(&[0u8; 16]).expect("SM4 key size must be 16 bytes");
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
