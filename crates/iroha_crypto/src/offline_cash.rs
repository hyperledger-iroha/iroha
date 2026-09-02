//! Low-level Offline Cash V1 encrypted-credit primitives.
//!
//! This module deliberately owns cryptography but no monetary authority. Its
//! callers must run it inside a qualified, non-forking hardware provider and
//! bind its inputs to the released Offline Cash circuits and hardware journal.
//! Successful AEAD authentication alone never authorizes minting, receiving,
//! spending, or redemption.

use std::vec::Vec;

use aead::{Aead as _, KeyInit as _, Payload};
use chacha20poly1305::XChaCha20Poly1305;
use hkdf::Hkdf;
use sha2::Sha256;
use thiserror::Error;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::kex::is_x25519_low_order_public_key;

/// Exact X25519 private/public-key width used by Offline Cash V1.
pub const OFFLINE_CASH_X25519_KEY_BYTES_V1: usize = 32;
/// Exact XChaCha20-Poly1305 key width used by Offline Cash V1.
pub const OFFLINE_CASH_XCHACHA20POLY1305_KEY_BYTES_V1: usize = 32;
/// Exact XChaCha20-Poly1305 nonce width used by Offline Cash V1.
pub const OFFLINE_CASH_XCHACHA20POLY1305_NONCE_BYTES_V1: usize = 24;

/// Failure in the low-level Offline Cash encrypted-credit primitive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum OfflineCashCreditCryptoErrorV1 {
    /// A local raw X25519 secret was the forbidden all-zero value.
    #[error("Offline Cash X25519 private key is all zero")]
    ZeroPrivateKey,
    /// A remote X25519 public key was low-order.
    #[error("Offline Cash X25519 public key is low-order")]
    LowOrderPublicKey,
    /// X25519 produced the forbidden all-zero contributory result.
    #[error("Offline Cash X25519 shared secret is all zero")]
    AllZeroSharedSecret,
    /// The supplied HKDF context was empty or could not derive the exact key.
    #[error("Offline Cash encrypted-credit KDF failed")]
    Kdf,
    /// XChaCha20-Poly1305 sealing failed.
    #[error("Offline Cash encrypted-credit sealing failed")]
    SealFailed,
    /// XChaCha20-Poly1305 authentication or opening failed.
    #[error("Offline Cash encrypted-credit opening failed")]
    OpenFailed,
}

/// Ciphertext plus the public half of the fresh sender ephemeral key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineCashCreditCiphertextV1 {
    /// Fresh X25519 public key corresponding to the supplied ephemeral secret.
    pub ephemeral_public_key: [u8; OFFLINE_CASH_X25519_KEY_BYTES_V1],
    /// XChaCha20-Poly1305 ciphertext followed by its 16-byte tag.
    pub ciphertext_and_tag: Vec<u8>,
}

/// Derive an X25519 public key from checked raw private material.
///
/// This helper is intended for a qualified provider that must compare a
/// non-exportable recipient key handle with its signed public projection.
/// It does not make the key or caller an authorized Offline Cash provider.
///
/// # Errors
///
/// Returns [`OfflineCashCreditCryptoErrorV1::ZeroPrivateKey`] for the
/// explicitly forbidden all-zero raw private value.
pub fn offline_cash_x25519_public_key_v1(
    private_key: &[u8; OFFLINE_CASH_X25519_KEY_BYTES_V1],
) -> Result<[u8; OFFLINE_CASH_X25519_KEY_BYTES_V1], OfflineCashCreditCryptoErrorV1> {
    let private_key = checked_private_key(private_key)?;
    Ok(X25519PublicKey::from(&*private_key).to_bytes())
}

/// Seal one canonical Offline Cash credit opening with explicit provider entropy.
///
/// `kdf_salt`, `kdf_info`, and `aad` must be the exact values derived by the
/// data-model helpers from the signed public credit context. The ephemeral
/// secret and nonce must be fresh, unpredictable values generated and retained
/// only inside the qualified provider boundary.
///
/// # Errors
///
/// Returns a typed error for inert or low-order key material, a non-contributory
/// X25519 result, invalid KDF context, or an AEAD failure.
#[allow(clippy::too_many_arguments)]
pub fn seal_offline_cash_credit_bytes_v1(
    recipient_public_key: [u8; OFFLINE_CASH_X25519_KEY_BYTES_V1],
    ephemeral_private_key: &[u8; OFFLINE_CASH_X25519_KEY_BYTES_V1],
    nonce: &[u8; OFFLINE_CASH_XCHACHA20POLY1305_NONCE_BYTES_V1],
    kdf_salt: &[u8; 32],
    kdf_info: &[u8],
    canonical_plaintext: &[u8],
    canonical_aad: &[u8],
) -> Result<OfflineCashCreditCiphertextV1, OfflineCashCreditCryptoErrorV1> {
    let ephemeral_private_key = checked_private_key(ephemeral_private_key)?;
    let recipient_public_key = checked_public_key(recipient_public_key)?;
    let ephemeral_public_key = X25519PublicKey::from(&*ephemeral_private_key).to_bytes();
    let key = derive_key(
        &ephemeral_private_key,
        &recipient_public_key,
        kdf_salt,
        kdf_info,
    )?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref())
        .map_err(|_| OfflineCashCreditCryptoErrorV1::SealFailed)?;
    let nonce = aead::Nonce::<XChaCha20Poly1305>::try_from(nonce.as_slice())
        .map_err(|_| OfflineCashCreditCryptoErrorV1::SealFailed)?;
    let ciphertext_and_tag = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: canonical_plaintext,
                aad: canonical_aad,
            },
        )
        .map_err(|_| OfflineCashCreditCryptoErrorV1::SealFailed)?;
    Ok(OfflineCashCreditCiphertextV1 {
        ephemeral_public_key,
        ciphertext_and_tag,
    })
}

/// Authenticate and open one canonical Offline Cash credit ciphertext.
///
/// `kdf_salt`, `kdf_info`, and `aad` must be recomputed from the exact public
/// envelope and signed credit context. Returned plaintext is zeroized on drop;
/// callers must decode and retain it only inside the qualified provider.
///
/// # Errors
///
/// Returns a typed error for inert or low-order key material, a non-contributory
/// X25519 result, invalid KDF context, or failed AEAD authentication.
#[allow(clippy::too_many_arguments)]
pub fn open_offline_cash_credit_bytes_v1(
    recipient_private_key: &[u8; OFFLINE_CASH_X25519_KEY_BYTES_V1],
    ephemeral_public_key: [u8; OFFLINE_CASH_X25519_KEY_BYTES_V1],
    nonce: &[u8; OFFLINE_CASH_XCHACHA20POLY1305_NONCE_BYTES_V1],
    kdf_salt: &[u8; 32],
    kdf_info: &[u8],
    ciphertext_and_tag: &[u8],
    canonical_aad: &[u8],
) -> Result<Zeroizing<Vec<u8>>, OfflineCashCreditCryptoErrorV1> {
    let recipient_private_key = checked_private_key(recipient_private_key)?;
    let ephemeral_public_key = checked_public_key(ephemeral_public_key)?;
    let key = derive_key(
        &recipient_private_key,
        &ephemeral_public_key,
        kdf_salt,
        kdf_info,
    )?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref())
        .map_err(|_| OfflineCashCreditCryptoErrorV1::OpenFailed)?;
    let nonce = aead::Nonce::<XChaCha20Poly1305>::try_from(nonce.as_slice())
        .map_err(|_| OfflineCashCreditCryptoErrorV1::OpenFailed)?;
    let plaintext = cipher
        .decrypt(
            &nonce,
            Payload {
                msg: ciphertext_and_tag,
                aad: canonical_aad,
            },
        )
        .map_err(|_| OfflineCashCreditCryptoErrorV1::OpenFailed)?;
    Ok(Zeroizing::new(plaintext))
}

fn checked_private_key(
    private_key: &[u8; OFFLINE_CASH_X25519_KEY_BYTES_V1],
) -> Result<Zeroizing<StaticSecret>, OfflineCashCreditCryptoErrorV1> {
    if private_key.iter().all(|byte| *byte == 0) {
        return Err(OfflineCashCreditCryptoErrorV1::ZeroPrivateKey);
    }
    Ok(Zeroizing::new(StaticSecret::from(*private_key)))
}

fn checked_public_key(
    public_key: [u8; OFFLINE_CASH_X25519_KEY_BYTES_V1],
) -> Result<X25519PublicKey, OfflineCashCreditCryptoErrorV1> {
    let public_key = X25519PublicKey::from(public_key);
    if is_x25519_low_order_public_key(&public_key) {
        return Err(OfflineCashCreditCryptoErrorV1::LowOrderPublicKey);
    }
    Ok(public_key)
}

fn derive_key(
    local_private_key: &StaticSecret,
    remote_public_key: &X25519PublicKey,
    kdf_salt: &[u8; 32],
    kdf_info: &[u8],
) -> Result<
    Zeroizing<[u8; OFFLINE_CASH_XCHACHA20POLY1305_KEY_BYTES_V1]>,
    OfflineCashCreditCryptoErrorV1,
> {
    if kdf_info.is_empty() {
        return Err(OfflineCashCreditCryptoErrorV1::Kdf);
    }
    let shared_secret = Zeroizing::new(
        local_private_key
            .diffie_hellman(remote_public_key)
            .to_bytes(),
    );
    if shared_secret.iter().all(|byte| *byte == 0) {
        return Err(OfflineCashCreditCryptoErrorV1::AllZeroSharedSecret);
    }
    let hkdf = Hkdf::<Sha256>::new(Some(kdf_salt), shared_secret.as_ref());
    let mut key = Zeroizing::new([0_u8; OFFLINE_CASH_XCHACHA20POLY1305_KEY_BYTES_V1]);
    hkdf.expand(kdf_info, key.as_mut())
        .map_err(|_| OfflineCashCreditCryptoErrorV1::Kdf)?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use curve25519_dalek::constants::EIGHT_TORSION;

    const ALICE_PRIVATE: [u8; 32] =
        hex_literal::hex!("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
    const ALICE_PUBLIC: [u8; 32] =
        hex_literal::hex!("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a");
    const BOB_PRIVATE: [u8; 32] =
        hex_literal::hex!("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb");
    const BOB_PUBLIC: [u8; 32] =
        hex_literal::hex!("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f");

    #[test]
    fn rfc_7748_public_keys_match() {
        assert_eq!(
            offline_cash_x25519_public_key_v1(&ALICE_PRIVATE).expect("Alice private key"),
            ALICE_PUBLIC
        );
        assert_eq!(
            offline_cash_x25519_public_key_v1(&BOB_PRIVATE).expect("Bob private key"),
            BOB_PUBLIC
        );
    }

    #[test]
    fn explicit_material_roundtrips() {
        let nonce = [0xA5; 24];
        let salt = [0xB6; 32];
        let info = b"offline-cash-test-info";
        let aad = b"offline-cash-test-aad";
        let plaintext = b"canonical private credit opening";
        let sealed = seal_offline_cash_credit_bytes_v1(
            BOB_PUBLIC,
            &ALICE_PRIVATE,
            &nonce,
            &salt,
            info,
            plaintext,
            aad,
        )
        .expect("seal");
        assert_eq!(sealed.ephemeral_public_key, ALICE_PUBLIC);
        assert_eq!(
            sealed.ciphertext_and_tag,
            hex_literal::hex!(
                "8e0abea022b4fc8fa6c3f3d9e8c1d5e5c2f7e1990d5afa834ef24b32d47932ab359e4a36750231b7d49f37d17ae4d926"
            )
        );
        let opened = open_offline_cash_credit_bytes_v1(
            &BOB_PRIVATE,
            sealed.ephemeral_public_key,
            &nonce,
            &salt,
            info,
            &sealed.ciphertext_and_tag,
            aad,
        )
        .expect("open");
        assert_eq!(opened.as_slice(), plaintext);
    }

    #[test]
    fn zero_secret_and_every_low_order_public_key_fail_closed() {
        assert_eq!(
            offline_cash_x25519_public_key_v1(&[0; 32]),
            Err(OfflineCashCreditCryptoErrorV1::ZeroPrivateKey)
        );
        let mut low_order_encodings = EIGHT_TORSION
            .iter()
            .map(|point| point.to_montgomery().0)
            .collect::<Vec<_>>();
        low_order_encodings.sort_unstable();
        low_order_encodings.dedup();
        assert!(low_order_encodings.len() > 1);
        for low_order in low_order_encodings {
            let error = seal_offline_cash_credit_bytes_v1(
                low_order,
                &ALICE_PRIVATE,
                &[1; 24],
                &[2; 32],
                b"info",
                b"plaintext",
                b"aad",
            )
            .expect_err("low-order recipient key");
            assert_eq!(error, OfflineCashCreditCryptoErrorV1::LowOrderPublicKey);
        }
    }

    #[test]
    fn authentication_rejects_tampering_and_wrong_context() {
        let nonce = [0xA5; 24];
        let salt = [0xB6; 32];
        let sealed = seal_offline_cash_credit_bytes_v1(
            BOB_PUBLIC,
            &ALICE_PRIVATE,
            &nonce,
            &salt,
            b"info",
            b"plaintext",
            b"aad",
        )
        .expect("seal");
        let mut tampered = sealed.ciphertext_and_tag.clone();
        tampered[0] ^= 1;
        assert_eq!(
            open_offline_cash_credit_bytes_v1(
                &BOB_PRIVATE,
                sealed.ephemeral_public_key,
                &nonce,
                &salt,
                b"info",
                &tampered,
                b"aad",
            ),
            Err(OfflineCashCreditCryptoErrorV1::OpenFailed)
        );
        assert_eq!(
            open_offline_cash_credit_bytes_v1(
                &BOB_PRIVATE,
                sealed.ephemeral_public_key,
                &nonce,
                &salt,
                b"different-info",
                &sealed.ciphertext_and_tag,
                b"aad",
            ),
            Err(OfflineCashCreditCryptoErrorV1::OpenFailed)
        );
        assert_eq!(
            open_offline_cash_credit_bytes_v1(
                &BOB_PRIVATE,
                sealed.ephemeral_public_key,
                &nonce,
                &salt,
                b"info",
                &sealed.ciphertext_and_tag,
                b"different-aad",
            ),
            Err(OfflineCashCreditCryptoErrorV1::OpenFailed)
        );
    }
}
