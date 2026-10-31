//! Borrowed ML-DSA-65 signing with strict key admission and direct OS entropy.
use super::{MlDsaError, MlDsaSuite, backend};
use rand_core::TryRng;
use zeroize::Zeroizing;

/// Sign using a canonical ML-DSA-65 secret key, fresh OS coins and an empty context.
///
/// # Errors
/// Returns an error for malformed secret keys or unavailable OS entropy.
pub fn sign_mldsa65_detached(message: &[u8], secret_key: &[u8]) -> Result<Vec<u8>, MlDsaError> {
    MlDsaSuite::MlDsa65.validate_secret_key(secret_key)?;
    let mut coins = Zeroizing::new([0_u8; 32]);
    rand::rngs::SysRng
        .try_fill_bytes(coins.as_mut())
        .map_err(|_| crate::RngError)?;
    backend::sign(MlDsaSuite::MlDsa65, secret_key, &[], message, &coins)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{generate_mldsa_keypair_from_fips_seed, verify_mldsa, verify_mldsa65_detached};
    #[test]
    fn borrowed_signing_preserves_empty_context_verdicts() {
        let suite = MlDsaSuite::MlDsa65;
        let keypair = generate_mldsa_keypair_from_fips_seed(suite, &[0x92; 32]).unwrap();
        for message in [&b""[..], &b"ML-DSA ISO 20022 signing dispatch"[..]] {
            let signature = sign_mldsa65_detached(message, keypair.secret_key()).unwrap();
            assert_eq!(signature.len(), suite.signature_len());
            verify_mldsa65_detached(&signature, message, keypair.public_key()).unwrap();
            verify_mldsa(suite, keypair.public_key(), &[], message, &signature).unwrap();
            assert!(
                verify_mldsa(
                    suite,
                    keypair.public_key(),
                    b"different context",
                    message,
                    &signature
                )
                .is_err()
            );
            assert!(verify_mldsa65_detached(&signature, b"changed", keypair.public_key()).is_err());
        }
        assert!(sign_mldsa65_detached(b"", &[]).is_err());
    }
}
