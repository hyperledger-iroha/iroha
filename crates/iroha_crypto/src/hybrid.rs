//! Hybrid KEM/DEM helpers powering the SoraFS payload envelope (SF-4b).
//!
//! The construction combines a classical X25519 ECDH exchange with ML-KEM-768
//! (Kyber) and feeds the concatenated shared secrets into an HKDF-SHA3-256
//! derive step. The resulting 32-byte key material is suitable for
//! ChaCha20-Poly1305 while the secondary output provides a deterministic
//! re-key secret so callers can rotate envelopes without advertising new
//! long-term public keys.

use core::{fmt, str::FromStr};

use hkdf::Hkdf;
use rand::{CryptoRng, RngCore};
use sha3::{Digest, Sha3_256};
use soranet_pq::{MlKemSuite, decapsulate_mlkem, encapsulate_mlkem, generate_mlkem_keypair};
use thiserror::Error;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};
use zeroize::Zeroizing;

const SUITE_KDF_SALT_V1: &[u8] = b"sorafs.hybrid.kem.hkdf:v1";
const SUITE_KDF_INFO_V1: &[u8] = b"sorafs.hybrid.kem.material:v1";
const SUITE_REKEY_INFO_V1: &[u8] = b"sorafs.hybrid.kem.rekey:v1";
const HYBRID_KEM_SUITE: MlKemSuite = MlKemSuite::MlKem768;

/// Supported hybrid suites for payload envelopes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HybridSuite {
    /// X25519 ECDH + ML-KEM-768 (Kyber) feeding ChaCha20-Poly1305.
    X25519MlKem768ChaCha20Poly1305,
}

impl HybridSuite {
    #[must_use]
    fn hkdf_salt(self) -> &'static [u8] {
        match self {
            HybridSuite::X25519MlKem768ChaCha20Poly1305 => SUITE_KDF_SALT_V1,
        }
    }

    #[must_use]
    fn hkdf_info(self) -> &'static [u8] {
        match self {
            HybridSuite::X25519MlKem768ChaCha20Poly1305 => SUITE_KDF_INFO_V1,
        }
    }

    #[must_use]
    fn rekey_info(self) -> &'static [u8] {
        match self {
            HybridSuite::X25519MlKem768ChaCha20Poly1305 => SUITE_REKEY_INFO_V1,
        }
    }

    #[must_use]
    fn description(self) -> &'static str {
        match self {
            HybridSuite::X25519MlKem768ChaCha20Poly1305 => "x25519-mlkem768-chacha20poly1305",
        }
    }
}

impl fmt::Display for HybridSuite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl FromStr for HybridSuite {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "x25519-mlkem768-chacha20poly1305" => Ok(Self::X25519MlKem768ChaCha20Poly1305),
            _ => Err(()),
        }
    }
}

/// Errors that may occur while working with the hybrid suite helpers.
#[derive(Debug, Error, PartialEq, Eq, Clone, Copy)]
pub enum HybridError {
    /// Invalid X25519 public key length encountered.
    #[error("invalid x25519 public key length (expected {expected}, found {found})")]
    InvalidX25519PublicKeyLength {
        /// Expected byte length.
        expected: usize,
        /// Observed byte length.
        found: usize,
    },
    /// Invalid X25519 secret key length encountered.
    #[error("invalid x25519 secret key length (expected {expected}, found {found})")]
    InvalidX25519SecretKeyLength {
        /// Expected byte length.
        expected: usize,
        /// Observed byte length.
        found: usize,
    },
    /// X25519 shared secret resolved to the all-zero value (low-order public key).
    #[error("x25519 shared secret is all-zero (invalid public key)")]
    InvalidX25519SharedSecret,
    /// Kyber public key did not match the expected length.
    #[error("invalid kyber public key length (expected {expected}, found {found})")]
    InvalidKyberPublicKeyLength {
        /// Expected byte length.
        expected: usize,
        /// Observed byte length.
        found: usize,
    },
    /// Kyber public key bytes were rejected by the crypto backend.
    #[error("kyber public key bytes rejected")]
    InvalidKyberPublicKey,
    /// Kyber secret key did not match the expected length.
    #[error("invalid kyber secret key length (expected {expected}, found {found})")]
    InvalidKyberSecretKeyLength {
        /// Expected byte length.
        expected: usize,
        /// Observed byte length.
        found: usize,
    },
    /// Kyber secret key bytes were rejected by the crypto backend.
    #[error("kyber secret key bytes rejected")]
    InvalidKyberSecretKey,
    /// Kyber ciphertext bytes were rejected by the crypto backend.
    #[error("kyber ciphertext bytes rejected")]
    InvalidKyberCiphertext,
    /// HKDF expand step failed due to a length mismatch.
    #[error("hkdf expand failed")]
    InvalidHkdfLength,
}

/// Hybrid public key combining X25519 and ML-KEM material.
#[derive(Clone)]
pub struct HybridPublicKey {
    x25519: X25519PublicKey,
    kyber: Vec<u8>,
}

impl HybridPublicKey {
    /// Create a [`HybridPublicKey`] from raw component bytes.
    ///
    /// # Errors
    ///
    /// Returns [`HybridError`] when either component does not match the expected
    /// length or fails validation by the underlying curve/KEM implementation.
    pub fn from_bytes(
        x25519: impl AsRef<[u8]>,
        kyber: impl AsRef<[u8]>,
    ) -> Result<Self, HybridError> {
        let x25519_bytes = x25519.as_ref();
        if x25519_bytes.len() != 32 {
            return Err(HybridError::InvalidX25519PublicKeyLength {
                expected: 32,
                found: x25519_bytes.len(),
            });
        }
        let mut x25519_array = [0_u8; 32];
        x25519_array.copy_from_slice(x25519_bytes);
        let x25519 = X25519PublicKey::from(x25519_array);

        let kyber_bytes = kyber.as_ref();
        let expected_len = HYBRID_KEM_SUITE.public_key_len();
        if kyber_bytes.len() != expected_len {
            return Err(HybridError::InvalidKyberPublicKeyLength {
                expected: expected_len,
                found: kyber_bytes.len(),
            });
        }
        HYBRID_KEM_SUITE
            .validate_public_key(kyber_bytes)
            .map_err(|_| HybridError::InvalidKyberPublicKey)?;

        Ok(Self {
            x25519,
            kyber: kyber_bytes.to_vec(),
        })
    }

    /// Return the contained X25519 public key.
    #[must_use]
    pub fn x25519(&self) -> &X25519PublicKey {
        &self.x25519
    }

    /// Return the contained ML-KEM public key bytes.
    #[must_use]
    pub fn kyber_bytes(&self) -> &[u8] {
        &self.kyber
    }

    /// Return the X25519 public key bytes.
    #[must_use]
    pub fn x25519_bytes(&self) -> [u8; 32] {
        self.x25519.to_bytes()
    }
}

impl fmt::Debug for HybridPublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut digest = Sha3_256::new();
        digest.update(&self.kyber);
        let fingerprint = hex::encode(digest.finalize());
        f.debug_struct("HybridPublicKey")
            .field("suite", &HybridSuite::X25519MlKem768ChaCha20Poly1305)
            .field("x25519", &hex::encode(self.x25519.to_bytes()))
            .field("kyber_fingerprint", &fingerprint)
            .finish()
    }
}

/// Hybrid secret key pairing the X25519 scalar with the Kyber secret.
pub struct HybridSecretKey {
    x25519: StaticSecret,
    kyber: Zeroizing<Vec<u8>>,
    public: HybridPublicKey,
}

impl HybridSecretKey {
    /// Construct a secret key from component bytes.
    ///
    /// # Errors
    ///
    /// Returns [`HybridError`] when any of the component byte strings has an
    /// unexpected length or fails decoding by the X25519 or Kyber backends.
    pub fn from_bytes(
        x25519: impl AsRef<[u8]>,
        kyber: impl AsRef<[u8]>,
    ) -> Result<Self, HybridError> {
        let x25519_bytes = x25519.as_ref();
        if x25519_bytes.len() != 32 {
            return Err(HybridError::InvalidX25519SecretKeyLength {
                expected: 32,
                found: x25519_bytes.len(),
            });
        }
        let mut x25519_array = Zeroizing::new([0_u8; 32]);
        x25519_array.copy_from_slice(x25519_bytes);
        let x25519_secret = StaticSecret::from(*x25519_array);
        let x25519_public = X25519PublicKey::from(&x25519_secret);

        let kyber_bytes = kyber.as_ref();
        let expected_len = HYBRID_KEM_SUITE.secret_key_len();
        if kyber_bytes.len() != expected_len {
            return Err(HybridError::InvalidKyberSecretKeyLength {
                expected: expected_len,
                found: kyber_bytes.len(),
            });
        }
        HYBRID_KEM_SUITE
            .validate_secret_key(kyber_bytes)
            .map_err(|_| HybridError::InvalidKyberSecretKey)?;
        let kyber_secret = Zeroizing::new(kyber_bytes.to_vec());

        // Kyber secret keys embed the public key in their trailing bytes per PQClean.
        let secret_bytes = kyber_secret.as_slice();
        let kyber_public_len = HYBRID_KEM_SUITE.public_key_len();
        let sym_bytes = HYBRID_KEM_SUITE.shared_secret_len();
        let public_offset = secret_bytes
            .len()
            .checked_sub(kyber_public_len + (2 * sym_bytes))
            .ok_or(HybridError::InvalidKyberSecretKey)?;
        let kyber_public_slice = &secret_bytes[public_offset..public_offset + kyber_public_len];
        let public = HybridPublicKey::from_bytes(x25519_public.to_bytes(), kyber_public_slice)?;

        Ok(Self {
            x25519: x25519_secret,
            kyber: kyber_secret,
            public,
        })
    }

    /// Return the public counterpart.
    #[must_use]
    pub fn public(&self) -> &HybridPublicKey {
        &self.public
    }

    /// Return the X25519 secret key.
    #[must_use]
    pub fn x25519(&self) -> &StaticSecret {
        &self.x25519
    }

    /// Return the Kyber secret key.
    #[must_use]
    pub fn kyber_bytes(&self) -> &[u8] {
        self.kyber.as_slice()
    }

    /// Export the component bytes. The Kyber secret key bytes include the embedded public key.
    #[must_use]
    pub fn to_bytes(&self) -> ([u8; 32], Vec<u8>) {
        (self.x25519.to_bytes(), self.kyber.as_slice().to_vec())
    }
}

impl fmt::Debug for HybridSecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HybridSecretKey")
            .field("suite", &HybridSuite::X25519MlKem768ChaCha20Poly1305)
            .finish_non_exhaustive()
    }
}

impl Clone for HybridSecretKey {
    fn clone(&self) -> Self {
        Self {
            x25519: self.x25519.clone(),
            kyber: Zeroizing::new(self.kyber.as_slice().to_vec()),
            public: self.public.clone(),
        }
    }
}

/// Key pair for the hybrid suite.
#[derive(Clone, Debug)]
pub struct HybridKeyPair {
    public: HybridPublicKey,
    secret: HybridSecretKey,
}

impl HybridKeyPair {
    /// Generate a fresh key pair using the provided RNG.
    pub fn generate<R>(rng: &mut R) -> Self
    where
        R: CryptoRng + RngCore,
    {
        let mut x25519_bytes = Zeroizing::new([0_u8; 32]);
        rng.fill_bytes(x25519_bytes.as_mut());
        let x25519_secret = StaticSecret::from(*x25519_bytes);
        let kem_pair = generate_mlkem_keypair(HYBRID_KEM_SUITE);
        let secret =
            HybridSecretKey::from_bytes(x25519_secret.to_bytes(), kem_pair.secret_key.as_slice())
                .expect("generated ML-KEM secret must be valid");
        let public = secret.public().clone();

        Self { public, secret }
    }

    /// Return the public component.
    #[must_use]
    pub fn public(&self) -> &HybridPublicKey {
        &self.public
    }

    /// Return the secret component.
    #[must_use]
    pub fn secret(&self) -> &HybridSecretKey {
        &self.secret
    }
}

/// Encapsulation output bundled with the sender's ephemeral state.
#[derive(Clone, PartialEq, Eq)]
pub struct HybridKemCiphertext {
    ephemeral_public: [u8; 32],
    kyber_ciphertext: Vec<u8>,
}

impl HybridKemCiphertext {
    /// Build a ciphertext bundle from raw parts.
    ///
    /// # Errors
    ///
    /// Returns [`HybridError`] when either the ephemeral X25519 component or the
    /// Kyber ciphertext fails validation.
    pub fn from_parts(
        ephemeral_public: impl AsRef<[u8]>,
        kyber_ciphertext: impl AsRef<[u8]>,
    ) -> Result<Self, HybridError> {
        let ephemeral_bytes = ephemeral_public.as_ref();
        if ephemeral_bytes.len() != 32 {
            return Err(HybridError::InvalidX25519PublicKeyLength {
                expected: 32,
                found: ephemeral_bytes.len(),
            });
        }
        let mut ephemeral_public_array = [0_u8; 32];
        ephemeral_public_array.copy_from_slice(ephemeral_bytes);

        let kyber_bytes = kyber_ciphertext.as_ref();
        let expected_ct_len = HYBRID_KEM_SUITE.ciphertext_len();
        if kyber_bytes.len() != expected_ct_len {
            return Err(HybridError::InvalidKyberCiphertext);
        }
        HYBRID_KEM_SUITE
            .validate_ciphertext(kyber_bytes)
            .map_err(|_| HybridError::InvalidKyberCiphertext)?;

        Ok(Self {
            ephemeral_public: ephemeral_public_array,
            kyber_ciphertext: kyber_bytes.to_vec(),
        })
    }

    /// Return the sender's ephemeral X25519 public key.
    #[must_use]
    pub fn ephemeral_public(&self) -> &[u8; 32] {
        &self.ephemeral_public
    }

    /// Return the Kyber ciphertext emitted during encapsulation.
    #[must_use]
    pub fn kyber_ciphertext(&self) -> &[u8] {
        self.kyber_ciphertext.as_slice()
    }
}

impl fmt::Debug for HybridKemCiphertext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HybridKemCiphertext")
            .field("ephemeral_public", &hex::encode(self.ephemeral_public))
            .field("kyber_ciphertext_len", &self.kyber_ciphertext.len())
            .finish()
    }
}

/// Symmetric material derived from the hybrid exchange.
pub struct DerivedSecret {
    encryption_key: Zeroizing<[u8; 32]>,
    rekey_secret: Zeroizing<[u8; 32]>,
}

impl DerivedSecret {
    /// Construct a new [`DerivedSecret`] from component arrays.
    fn new(encryption_key: [u8; 32], rekey_secret: [u8; 32]) -> Self {
        Self {
            encryption_key: Zeroizing::new(encryption_key),
            rekey_secret: Zeroizing::new(rekey_secret),
        }
    }

    /// Symmetric key for ChaCha20-Poly1305.
    #[must_use]
    pub fn encryption_key(&self) -> [u8; 32] {
        *self.encryption_key
    }

    /// Secondary secret used for deterministic rekey derivations.
    #[must_use]
    pub fn rekey_secret(&self) -> [u8; 32] {
        *self.rekey_secret
    }
}

impl Clone for DerivedSecret {
    fn clone(&self) -> Self {
        Self::new(self.encryption_key(), self.rekey_secret())
    }
}

impl fmt::Debug for DerivedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DerivedSecret")
            .field("suite", &HybridSuite::X25519MlKem768ChaCha20Poly1305)
            .finish_non_exhaustive()
    }
}

/// Derive symmetric material for the given recipient public key.
///
/// # Errors
///
/// Returns [`HybridError`] if encapsulation fails (for example, when the
/// X25519 shared secret is all-zero due to a low-order public key, Kyber
/// rejects the peer parameters, or HKDF expansion cannot produce the requested
/// output length).
pub fn encapsulate<R>(
    suite: HybridSuite,
    recipient: &HybridPublicKey,
    rng: &mut R,
) -> Result<(HybridKemCiphertext, DerivedSecret), HybridError>
where
    R: CryptoRng + RngCore,
{
    match suite {
        HybridSuite::X25519MlKem768ChaCha20Poly1305 => {
            let mut ephemeral_bytes = Zeroizing::new([0_u8; 32]);
            rng.fill_bytes(ephemeral_bytes.as_mut());
            let ephemeral_secret = StaticSecret::from(*ephemeral_bytes);
            let ephemeral_public = X25519PublicKey::from(&ephemeral_secret);
            let shared_ecdh = ephemeral_secret.diffie_hellman(recipient.x25519());
            if shared_ecdh.as_bytes().iter().all(|&byte| byte == 0) {
                return Err(HybridError::InvalidX25519SharedSecret);
            }

            let (kyber_shared, kyber_ciphertext) =
                encapsulate_mlkem(HYBRID_KEM_SUITE, recipient.kyber_bytes())
                    .map_err(|_| HybridError::InvalidKyberPublicKey)?;
            debug_assert_eq!(
                kyber_ciphertext.as_bytes().len(),
                HYBRID_KEM_SUITE.ciphertext_len()
            );
            let derived = derive_material(suite, shared_ecdh.as_bytes(), kyber_shared.as_bytes())?;

            let ciphertext = HybridKemCiphertext {
                ephemeral_public: ephemeral_public.to_bytes(),
                kyber_ciphertext: kyber_ciphertext.as_bytes().to_vec(),
            };

            Ok((ciphertext, derived))
        }
    }
}

/// Recover symmetric material from an encapsulated bundle.
///
/// # Errors
///
/// Returns [`HybridError`] if the ciphertext cannot be parsed, the X25519
/// shared secret is all-zero due to a low-order public key, Kyber decapsulation
/// fails, or HKDF expansion cannot produce the requested output length.
pub fn decapsulate(
    suite: HybridSuite,
    ciphertext: &HybridKemCiphertext,
    recipient: &HybridSecretKey,
) -> Result<DerivedSecret, HybridError> {
    match suite {
        HybridSuite::X25519MlKem768ChaCha20Poly1305 => {
            let ephemeral_public = X25519PublicKey::from(*ciphertext.ephemeral_public());
            let shared_ecdh = recipient.x25519().diffie_hellman(&ephemeral_public);
            if shared_ecdh.as_bytes().iter().all(|&byte| byte == 0) {
                return Err(HybridError::InvalidX25519SharedSecret);
            }

            HYBRID_KEM_SUITE
                .validate_ciphertext(ciphertext.kyber_ciphertext())
                .map_err(|_| HybridError::InvalidKyberCiphertext)?;
            let kyber_secret = decapsulate_mlkem(
                HYBRID_KEM_SUITE,
                recipient.kyber_bytes(),
                ciphertext.kyber_ciphertext(),
            )
            .map_err(|_| HybridError::InvalidKyberCiphertext)?;

            derive_material(suite, shared_ecdh.as_bytes(), kyber_secret.as_bytes())
        }
    }
}

/// Run the HKDF extraction/expansion sequence for the hybrid suite.
///
/// # Errors
///
/// Returns [`HybridError::InvalidHkdfLength`] when the HKDF backend rejects the
/// requested output lengths.
fn derive_material(
    suite: HybridSuite,
    ecdh: &[u8],
    kyber: &[u8],
) -> Result<DerivedSecret, HybridError> {
    let mut ikm = Zeroizing::new(Vec::with_capacity(ecdh.len() + kyber.len()));
    ikm.extend_from_slice(ecdh);
    ikm.extend_from_slice(kyber);
    let hkdf = Hkdf::<Sha3_256>::new(Some(suite.hkdf_salt()), ikm.as_ref());

    let mut okm = Zeroizing::new([0_u8; 64]);
    hkdf.expand(suite.hkdf_info(), okm.as_mut())
        .map_err(|_| HybridError::InvalidHkdfLength)?;

    let mut encryption_key = [0_u8; 32];
    encryption_key.copy_from_slice(&okm[..32]);

    let mut rekey_buf = Zeroizing::new([0_u8; 32]);
    hkdf.expand(suite.rekey_info(), rekey_buf.as_mut())
        .map_err(|_| HybridError::InvalidHkdfLength)?;
    let mut rekey_secret = [0_u8; 32];
    rekey_secret.copy_from_slice(rekey_buf.as_ref());

    Ok(DerivedSecret::new(encryption_key, rekey_secret))
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng as _;
    use rand_chacha::ChaCha20Rng;

    use super::*;

    #[test]
    fn generated_keys_roundtrip() {
        let mut rng = ChaCha20Rng::from_seed([0x42; 32]);
        let pair = HybridKeyPair::generate(&mut rng);

        let (ciphertext, sender) = encapsulate(
            HybridSuite::X25519MlKem768ChaCha20Poly1305,
            pair.public(),
            &mut rng,
        )
        .expect("encapsulation succeeds");
        assert_eq!(
            ciphertext.kyber_ciphertext().len(),
            HYBRID_KEM_SUITE.ciphertext_len()
        );
        HYBRID_KEM_SUITE
            .validate_ciphertext(ciphertext.kyber_ciphertext())
            .expect("encapsulated ciphertext decodes");
        let receiver = decapsulate(
            HybridSuite::X25519MlKem768ChaCha20Poly1305,
            &ciphertext,
            pair.secret(),
        )
        .expect("decapsulation succeeds");

        assert_eq!(sender.encryption_key(), receiver.encryption_key());
        assert_eq!(sender.rekey_secret(), receiver.rekey_secret());
    }

    #[test]
    fn public_key_encoding_roundtrip() {
        let mut rng = ChaCha20Rng::from_seed([0x23; 32]);
        let pair = HybridKeyPair::generate(&mut rng);
        let encoded_pub = (
            pair.public().x25519_bytes(),
            pair.public().kyber_bytes().to_vec(),
        );
        let encoded_secret = pair.secret().to_bytes();

        let decoded_pub =
            HybridPublicKey::from_bytes(encoded_pub.0, encoded_pub.1).expect("public key parses");
        let decoded_secret = HybridSecretKey::from_bytes(encoded_secret.0, encoded_secret.1)
            .expect("secret key parses");

        assert_eq!(decoded_pub.x25519_bytes(), pair.public().x25519_bytes());
        assert_eq!(decoded_pub.kyber_bytes(), pair.public().kyber_bytes());
        assert_eq!(
            decoded_secret.public().x25519_bytes(),
            pair.public().x25519_bytes()
        );
        assert_eq!(
            decoded_secret.public().kyber_bytes(),
            pair.public().kyber_bytes()
        );
    }

    #[test]
    fn encapsulate_rejects_low_order_x25519_public_key() {
        let mut rng = ChaCha20Rng::from_seed([0x77; 32]);
        let pair = HybridKeyPair::generate(&mut rng);
        let bad_public = HybridPublicKey::from_bytes([0u8; 32], pair.public().kyber_bytes())
            .expect("public key parses");
        let err = encapsulate(
            HybridSuite::X25519MlKem768ChaCha20Poly1305,
            &bad_public,
            &mut rng,
        )
        .expect_err("low-order public key must be rejected");
        assert_eq!(err, HybridError::InvalidX25519SharedSecret);
    }

    #[test]
    fn decapsulate_rejects_low_order_ephemeral_public_key() {
        let mut rng = ChaCha20Rng::from_seed([0x19; 32]);
        let pair = HybridKeyPair::generate(&mut rng);
        let (mut ciphertext, _sender) = encapsulate(
            HybridSuite::X25519MlKem768ChaCha20Poly1305,
            pair.public(),
            &mut rng,
        )
        .expect("encapsulation succeeds");
        ciphertext.ephemeral_public = [0u8; 32];
        let err = decapsulate(
            HybridSuite::X25519MlKem768ChaCha20Poly1305,
            &ciphertext,
            pair.secret(),
        )
        .expect_err("low-order public key must be rejected");
        assert_eq!(err, HybridError::InvalidX25519SharedSecret);
    }
}
