//! Low-level KAGEMUSHA V1 encrypted-credit and deterministic-recovery primitives.
//!
//! This module deliberately owns cryptography but no monetary authority. Its
//! callers must run it inside a qualified, non-forking hardware provider and
//! bind its inputs to the released KAGEMUSHA circuits and hardware journal.
//! Successful AEAD authentication alone never authorizes minting, receiving,
//! spending, or redemption.

use std::vec::Vec;

use aead::{Aead as _, KeyInit as _, Payload};
use chacha20poly1305::XChaCha20Poly1305;
use hkdf::Hkdf;
use sha2::Sha256;
use thiserror::Error;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};
use zeroize::{Zeroize as _, Zeroizing};

use crate::kex::is_x25519_low_order_public_key;

/// Exact X25519 private/public-key width used by KAGEMUSHA V1.
pub const KAGEMUSHA_X25519_KEY_BYTES_V1: usize = 32;
/// Exact XChaCha20-Poly1305 key width used by KAGEMUSHA V1.
pub const KAGEMUSHA_XCHACHA20POLY1305_KEY_BYTES_V1: usize = 32;
/// Exact XChaCha20-Poly1305 nonce width used by KAGEMUSHA V1.
pub const KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1: usize = 24;

const RECOVERY_RNG_SALT_V1: &[u8] = b"iroha:kagemusha:v1:recovery-seed\0";
const RECOVERY_RNG_INFO_V1: &[u8] = b"iroha:kagemusha:v1:recovery-rng\0";

/// Failure to accept secret recovery material or derive a purpose-specific stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum KagemushaRecoverySeedErrorV1 {
    /// The unsealed seed was the explicitly forbidden all-zero value.
    #[error("KAGEMUSHA recovery seed is all zero")]
    ZeroSeed,
    /// A stream must name its nonempty protocol purpose.
    #[error("KAGEMUSHA recovery RNG purpose is empty")]
    EmptyPurpose,
    /// HKDF could not derive the fixed-width stream seed.
    #[error("KAGEMUSHA recovery RNG derivation failed")]
    Kdf,
}

/// Secret entropy unsealed for one immutable prepared hardware operation.
///
/// The qualified provider must originally generate at least 256 bits of entropy
/// for each operation and authenticate its binding when unsealing. An encrypted
/// seed blob, public preparation identifier, password, or other predictable
/// value is **not** unsealed entropy. Construction only rejects the all-zero
/// sentinel; it cannot establish entropy quality or hardware authorization.
///
/// This type deliberately has no clone, codec, or raw-byte accessor. Its secret
/// storage is zeroized on drop and its debug representation is redacted.
pub struct KagemushaRecoverySeedV1(Zeroizing<[u8; 32]>);

impl core::fmt::Debug for KagemushaRecoverySeedV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("KagemushaRecoverySeedV1([REDACTED])")
    }
}

impl KagemushaRecoverySeedV1 {
    /// Accept secret bytes authenticated and unsealed by the qualified provider.
    ///
    /// The caller must satisfy this type's entropy and per-operation binding
    /// requirements; this constructor does not authenticate hardware evidence.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaRecoverySeedErrorV1::ZeroSeed`] for all-zero material.
    pub fn from_unsealed(mut unsealed: [u8; 32]) -> Result<Self, KagemushaRecoverySeedErrorV1> {
        if unsealed.iter().all(|byte| *byte == 0) {
            return Err(KagemushaRecoverySeedErrorV1::ZeroSeed);
        }
        let seed = Zeroizing::new(unsealed);
        unsealed.zeroize();
        Ok(Self(seed))
    }

    /// Derive a repeatable secret stream for one exact protocol purpose/context.
    ///
    /// HKDF-SHA256 uses a fixed KAGEMUSHA V1 recovery salt and the unambiguous
    /// info transcript `domain || purpose_length_u64_le || purpose || context32`.
    /// The resulting zeroizing 32-byte seed initializes the crate's deterministic
    /// ChaCha RNG. The same operation seed, purpose, and context reproduce the
    /// same stream; each distinct proof/fold/encryption purpose must use a
    /// separate label and bind all protocol-relevant ordered inputs in context.
    /// The caller must retain the returned secret RNG inside the provider.
    ///
    /// # Errors
    ///
    /// Rejects an empty purpose or an HKDF derivation failure.
    pub fn rng(
        &self,
        purpose: &[u8],
        context_digest: &[u8; 32],
    ) -> Result<
        impl rand_core_06::RngCore + rand_core_06::CryptoRng + rand_core::CryptoRng,
        KagemushaRecoverySeedErrorV1,
    > {
        if purpose.is_empty() {
            return Err(KagemushaRecoverySeedErrorV1::EmptyPurpose);
        }
        let purpose_length = (purpose.len() as u64).to_le_bytes();
        let hkdf = Hkdf::<Sha256>::new(Some(RECOVERY_RNG_SALT_V1), self.0.as_ref());
        let mut stream_seed = Zeroizing::new([0_u8; 32]);
        hkdf.expand_multi_info(
            &[
                RECOVERY_RNG_INFO_V1,
                &purpose_length,
                purpose,
                context_digest,
            ],
            stream_seed.as_mut(),
        )
        .map_err(|_| KagemushaRecoverySeedErrorV1::Kdf)?;
        Ok(crate::rng_from_seed_slice(stream_seed.as_ref()))
    }
}

/// Failure in the low-level KAGEMUSHA encrypted-credit primitive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum KagemushaCreditCryptoErrorV1 {
    /// A local raw X25519 secret was the forbidden all-zero value.
    #[error("KAGEMUSHA X25519 private key is all zero")]
    ZeroPrivateKey,
    /// A remote X25519 public key was low-order.
    #[error("KAGEMUSHA X25519 public key is low-order")]
    LowOrderPublicKey,
    /// X25519 produced the forbidden all-zero contributory result.
    #[error("KAGEMUSHA X25519 shared secret is all zero")]
    AllZeroSharedSecret,
    /// The supplied HKDF context was empty or could not derive the exact key.
    #[error("KAGEMUSHA encrypted-credit KDF failed")]
    Kdf,
    /// XChaCha20-Poly1305 sealing failed.
    #[error("KAGEMUSHA encrypted-credit sealing failed")]
    SealFailed,
    /// XChaCha20-Poly1305 authentication or opening failed.
    #[error("KAGEMUSHA encrypted-credit opening failed")]
    OpenFailed,
}

/// Ciphertext plus the public half of the fresh sender ephemeral key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaCreditCiphertextV1 {
    /// Fresh X25519 public key corresponding to the supplied ephemeral secret.
    pub ephemeral_public_key: [u8; KAGEMUSHA_X25519_KEY_BYTES_V1],
    /// XChaCha20-Poly1305 ciphertext followed by its 16-byte tag.
    pub ciphertext_and_tag: Vec<u8>,
}

/// Derive an X25519 public key from checked raw private material.
///
/// This helper is intended for a qualified provider that must compare a
/// non-exportable recipient key handle with its signed public projection.
/// It does not make the key or caller an authorized KAGEMUSHA provider.
///
/// # Errors
///
/// Returns [`KagemushaCreditCryptoErrorV1::ZeroPrivateKey`] for the
/// explicitly forbidden all-zero raw private value.
pub fn kagemusha_x25519_public_key_v1(
    private_key: &[u8; KAGEMUSHA_X25519_KEY_BYTES_V1],
) -> Result<[u8; KAGEMUSHA_X25519_KEY_BYTES_V1], KagemushaCreditCryptoErrorV1> {
    let private_key = checked_private_key(private_key)?;
    Ok(X25519PublicKey::from(&*private_key).to_bytes())
}

/// Seal one canonical KAGEMUSHA credit opening with explicit provider entropy.
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
pub fn seal_kagemusha_credit_bytes_v1(
    recipient_public_key: [u8; KAGEMUSHA_X25519_KEY_BYTES_V1],
    ephemeral_private_key: &[u8; KAGEMUSHA_X25519_KEY_BYTES_V1],
    nonce: &[u8; KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1],
    kdf_salt: &[u8; 32],
    kdf_info: &[u8],
    canonical_plaintext: &[u8],
    canonical_aad: &[u8],
) -> Result<KagemushaCreditCiphertextV1, KagemushaCreditCryptoErrorV1> {
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
        .map_err(|_| KagemushaCreditCryptoErrorV1::SealFailed)?;
    let nonce = aead::Nonce::<XChaCha20Poly1305>::try_from(nonce.as_slice())
        .map_err(|_| KagemushaCreditCryptoErrorV1::SealFailed)?;
    let ciphertext_and_tag = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: canonical_plaintext,
                aad: canonical_aad,
            },
        )
        .map_err(|_| KagemushaCreditCryptoErrorV1::SealFailed)?;
    Ok(KagemushaCreditCiphertextV1 {
        ephemeral_public_key,
        ciphertext_and_tag,
    })
}

/// Authenticate and open one canonical KAGEMUSHA credit ciphertext.
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
pub fn open_kagemusha_credit_bytes_v1(
    recipient_private_key: &[u8; KAGEMUSHA_X25519_KEY_BYTES_V1],
    ephemeral_public_key: [u8; KAGEMUSHA_X25519_KEY_BYTES_V1],
    nonce: &[u8; KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1],
    kdf_salt: &[u8; 32],
    kdf_info: &[u8],
    ciphertext_and_tag: &[u8],
    canonical_aad: &[u8],
) -> Result<Zeroizing<Vec<u8>>, KagemushaCreditCryptoErrorV1> {
    let recipient_private_key = checked_private_key(recipient_private_key)?;
    let ephemeral_public_key = checked_public_key(ephemeral_public_key)?;
    let key = derive_key(
        &recipient_private_key,
        &ephemeral_public_key,
        kdf_salt,
        kdf_info,
    )?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref())
        .map_err(|_| KagemushaCreditCryptoErrorV1::OpenFailed)?;
    let nonce = aead::Nonce::<XChaCha20Poly1305>::try_from(nonce.as_slice())
        .map_err(|_| KagemushaCreditCryptoErrorV1::OpenFailed)?;
    let plaintext = cipher
        .decrypt(
            &nonce,
            Payload {
                msg: ciphertext_and_tag,
                aad: canonical_aad,
            },
        )
        .map_err(|_| KagemushaCreditCryptoErrorV1::OpenFailed)?;
    Ok(Zeroizing::new(plaintext))
}

fn checked_private_key(
    private_key: &[u8; KAGEMUSHA_X25519_KEY_BYTES_V1],
) -> Result<Zeroizing<StaticSecret>, KagemushaCreditCryptoErrorV1> {
    if private_key.iter().all(|byte| *byte == 0) {
        return Err(KagemushaCreditCryptoErrorV1::ZeroPrivateKey);
    }
    Ok(Zeroizing::new(StaticSecret::from(*private_key)))
}

fn checked_public_key(
    public_key: [u8; KAGEMUSHA_X25519_KEY_BYTES_V1],
) -> Result<X25519PublicKey, KagemushaCreditCryptoErrorV1> {
    let public_key = X25519PublicKey::from(public_key);
    if is_x25519_low_order_public_key(&public_key) {
        return Err(KagemushaCreditCryptoErrorV1::LowOrderPublicKey);
    }
    Ok(public_key)
}

fn derive_key(
    local_private_key: &StaticSecret,
    remote_public_key: &X25519PublicKey,
    kdf_salt: &[u8; 32],
    kdf_info: &[u8],
) -> Result<Zeroizing<[u8; KAGEMUSHA_XCHACHA20POLY1305_KEY_BYTES_V1]>, KagemushaCreditCryptoErrorV1>
{
    if kdf_info.is_empty() {
        return Err(KagemushaCreditCryptoErrorV1::Kdf);
    }
    let shared_secret = Zeroizing::new(
        local_private_key
            .diffie_hellman(remote_public_key)
            .to_bytes(),
    );
    if shared_secret.iter().all(|byte| *byte == 0) {
        return Err(KagemushaCreditCryptoErrorV1::AllZeroSharedSecret);
    }
    let hkdf = Hkdf::<Sha256>::new(Some(kdf_salt), shared_secret.as_ref());
    let mut key = Zeroizing::new([0_u8; KAGEMUSHA_XCHACHA20POLY1305_KEY_BYTES_V1]);
    hkdf.expand(kdf_info, key.as_mut())
        .map_err(|_| KagemushaCreditCryptoErrorV1::Kdf)?;
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

    fn recovery_stream(
        seed: &KagemushaRecoverySeedV1,
        purpose: &[u8],
        context: &[u8; 32],
    ) -> [u8; 64] {
        let mut rng = seed.rng(purpose, context).expect("test-only recovery RNG");
        let mut bytes = [0_u8; 64];
        rand_core_06::RngCore::fill_bytes(&mut rng, &mut bytes);
        bytes
    }

    #[test]
    fn recovery_seed_rejects_zero_and_redacts_debug() {
        assert!(matches!(
            KagemushaRecoverySeedV1::from_unsealed([0; 32]),
            Err(KagemushaRecoverySeedErrorV1::ZeroSeed)
        ));
        // Predictable material is strictly a unit-test fixture, never provider entropy.
        let seed = KagemushaRecoverySeedV1::from_unsealed([0xA1; 32]).expect("test-only seed");
        assert_eq!(format!("{seed:?}"), "KagemushaRecoverySeedV1([REDACTED])");
        assert!(matches!(
            seed.rng(b"", &[0xB2; 32]),
            Err(KagemushaRecoverySeedErrorV1::EmptyPurpose)
        ));
    }

    #[test]
    fn recovery_stream_is_repeatable_and_isolates_seed_purpose_and_context() {
        // Fixed seeds model a test provider only and carry no production entropy claim.
        let seed = KagemushaRecoverySeedV1::from_unsealed([0xA1; 32]).expect("test-only seed");
        let other_seed =
            KagemushaRecoverySeedV1::from_unsealed([0xA2; 32]).expect("other test-only seed");
        let context = [0xB2; 32];
        let expected = recovery_stream(&seed, b"fold:eq", &context);
        assert_eq!(expected, recovery_stream(&seed, b"fold:eq", &context));
        assert_ne!(expected, recovery_stream(&other_seed, b"fold:eq", &context));
        assert_ne!(expected, recovery_stream(&seed, b"fold:ep", &context));
        assert_ne!(expected, recovery_stream(&seed, b"fold:eq", &[0xB3; 32]));
        assert_ne!(
            recovery_stream(&seed, b"a\0b", &context),
            recovery_stream(&seed, b"a", &context)
        );

        let mut modern_rng = seed.rng(b"fold:eq", &context).expect("modern RNG");
        let mut modern_bytes = [0_u8; 64];
        rand_core::RngCore::fill_bytes(&mut modern_rng, &mut modern_bytes);
        assert_eq!(expected, modern_bytes);
    }

    #[test]
    fn rfc_7748_public_keys_match() {
        assert_eq!(
            kagemusha_x25519_public_key_v1(&ALICE_PRIVATE).expect("Alice private key"),
            ALICE_PUBLIC
        );
        assert_eq!(
            kagemusha_x25519_public_key_v1(&BOB_PRIVATE).expect("Bob private key"),
            BOB_PUBLIC
        );
    }

    #[test]
    fn explicit_material_roundtrips() {
        let nonce = [0xA5; 24];
        let salt = [0xB6; 32];
        let info = b"kagemusha-test-info";
        let aad = b"kagemusha-test-aad";
        let plaintext = b"canonical private credit opening";
        let sealed = seal_kagemusha_credit_bytes_v1(
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
        let opened = open_kagemusha_credit_bytes_v1(
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
            kagemusha_x25519_public_key_v1(&[0; 32]),
            Err(KagemushaCreditCryptoErrorV1::ZeroPrivateKey)
        );
        let mut low_order_encodings = EIGHT_TORSION
            .iter()
            .map(|point| point.to_montgomery().0)
            .collect::<Vec<_>>();
        low_order_encodings.sort_unstable();
        low_order_encodings.dedup();
        assert!(low_order_encodings.len() > 1);
        for low_order in low_order_encodings {
            let error = seal_kagemusha_credit_bytes_v1(
                low_order,
                &ALICE_PRIVATE,
                &[1; 24],
                &[2; 32],
                b"info",
                b"plaintext",
                b"aad",
            )
            .expect_err("low-order recipient key");
            assert_eq!(error, KagemushaCreditCryptoErrorV1::LowOrderPublicKey);
        }
    }

    #[test]
    fn authentication_rejects_tampering_and_wrong_context() {
        let nonce = [0xA5; 24];
        let salt = [0xB6; 32];
        let sealed = seal_kagemusha_credit_bytes_v1(
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
            open_kagemusha_credit_bytes_v1(
                &BOB_PRIVATE,
                sealed.ephemeral_public_key,
                &nonce,
                &salt,
                b"info",
                &tampered,
                b"aad",
            ),
            Err(KagemushaCreditCryptoErrorV1::OpenFailed)
        );
        assert_eq!(
            open_kagemusha_credit_bytes_v1(
                &BOB_PRIVATE,
                sealed.ephemeral_public_key,
                &nonce,
                &salt,
                b"different-info",
                &sealed.ciphertext_and_tag,
                b"aad",
            ),
            Err(KagemushaCreditCryptoErrorV1::OpenFailed)
        );
        assert_eq!(
            open_kagemusha_credit_bytes_v1(
                &BOB_PRIVATE,
                sealed.ephemeral_public_key,
                &nonce,
                &salt,
                b"info",
                &sealed.ciphertext_and_tag,
                b"different-aad",
            ),
            Err(KagemushaCreditCryptoErrorV1::OpenFailed)
        );
    }
}
