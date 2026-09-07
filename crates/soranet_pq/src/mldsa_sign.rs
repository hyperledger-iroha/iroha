//! ML-DSA-65 signing with the existing pqcrypto key and entropy contracts.

use pqcrypto_mldsa::{ffi, mldsa65};
use pqcrypto_traits::sign::{DetachedSignature as _, SecretKey as _};

use super::verifier::upstream_verifier_available;

/// Sign with a typed ML-DSA-65 key and an empty FIPS 204 context.
///
/// Retains pqcrypto's length-only typed-key decoding and direct OS randomness
/// behavior for the ISO 20022 signing API. Canonical key admission remains the
/// caller's responsibility. Other signing APIs retain their existing explicit
/// key validation and hedged randomness. On AArch64 without NEON or SHA3, this
/// uses the public CLEAN byte interface with the same entropy source.
pub fn sign_mldsa65_detached(message: &[u8], secret_key: &mldsa65::SecretKey) -> Vec<u8> {
    sign_mldsa65_detached_with_upstream(message, secret_key, true)
}

fn sign_mldsa65_detached_with_upstream(
    message: &[u8],
    secret_key: &mldsa65::SecretKey,
    allow_upstream: bool,
) -> Vec<u8> {
    if allow_upstream && upstream_verifier_available() {
        return mldsa65::detached_sign(message, secret_key)
            .as_bytes()
            .to_vec();
    }
    let mut signature = [0_u8; mldsa65::signature_bytes()];
    let mut signature_len = 0_usize;
    // SAFETY: output has the exact suite capacity, the typed key has the exact
    // key length, and the borrowed message remains valid. The zero-length
    // context is identical to pqcrypto's detached_sign call. CLEAN writes only
    // within the signature buffer and sets the returned length to its size.
    let _status = unsafe {
        ffi::PQCLEAN_MLDSA65_CLEAN_crypto_sign_signature_ctx(
            signature.as_mut_ptr(),
            &mut signature_len,
            message.as_ptr(),
            message.len(),
            core::ptr::null(),
            0,
            secret_key.as_bytes().as_ptr(),
        )
    };
    // Match the upstream wrapper's initialized buffer/length, ignored status,
    // and safe prefix extraction. Its sole status error is an overlong context,
    // which is impossible here. Both backends use the same randombytes(32) call
    // and retain its existing fatal OS entropy failure behavior.
    signature[..signature_len].to_vec()
}

#[cfg(test)]
mod tests {
    use super::super::verifier::{verify_mldsa65_detached, verify65_ctx};
    use super::*;
    use crate::{MlDsaSuite, generate_mldsa_keypair_from_fips_seed};
    use pqcrypto_traits::sign::PublicKey as _;

    #[test]
    fn clean_and_automatic_signing_preserve_empty_context_verdicts() {
        let suite = MlDsaSuite::MlDsa65;
        let keypair = generate_mldsa_keypair_from_fips_seed(suite, &[0x92; 32])
            .expect("fixed portable keypair");
        let secret_key = mldsa65::SecretKey::from_bytes(keypair.secret_key()).unwrap();
        let public_key = mldsa65::PublicKey::from_bytes(keypair.public_key()).unwrap();
        for message in [&b""[..], &b"ML-DSA ISO 20022 signing dispatch"[..]] {
            for signature in [
                sign_mldsa65_detached_with_upstream(message, &secret_key, false),
                sign_mldsa65_detached_with_upstream(message, &secret_key, true),
                sign_mldsa65_detached(message, &secret_key),
            ] {
                assert_eq!(signature.len(), mldsa65::signature_bytes());
                let typed = mldsa65::DetachedSignature::from_bytes(&signature).unwrap();
                verify_mldsa65_detached(&typed, message, &public_key)
                    .expect("typed empty-context verification");
                verify65_ctx(&typed, message, &[], &public_key)
                    .expect("explicit empty-context verification");
                assert!(verify65_ctx(&typed, message, b"different context", &public_key).is_err());
                assert!(verify_mldsa65_detached(&typed, b"changed", &public_key).is_err());
            }
        }
    }
}
