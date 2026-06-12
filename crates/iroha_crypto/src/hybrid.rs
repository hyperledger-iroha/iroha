//! Hybrid KEM/DEM helpers powering the SoraFS payload envelope (SF-4b).
//!
//! The construction combines a classical X25519 ECDH exchange with ML-KEM-768
//! (Kyber) and feeds the concatenated shared secrets into an HKDF-SHA3-256
//! derive step together with a length-prefixed public transcript binding. The
//! resulting 32-byte key material is suitable for ChaCha20-Poly1305 while the
//! secondary output provides a deterministic re-key secret so callers can rotate
//! envelopes without advertising new long-term public keys.

use core::{fmt, str::FromStr};

use hkdf::Hkdf;
use rand_core::TryCryptoRng;
use sha3::{Digest, Sha3_256};
use soranet_pq::{
    HedgedRngSeed, MlKemSuite, decapsulate_mlkem, encapsulate_mlkem, hedged_chacha20_rng,
    try_generate_mlkem_keypair,
};
use thiserror::Error;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::kex::is_x25519_low_order_public_key;

const SUITE_KDF_SALT_V1: &[u8] = b"sorafs.hybrid.kem.hkdf:transcript-v1";
const SUITE_KDF_INFO_V1: &[u8] = b"sorafs.hybrid.kem.material:transcript-v1";
const SUITE_REKEY_INFO_V1: &[u8] = b"sorafs.hybrid.kem.rekey:transcript-v1";
const SUITE_TRANSCRIPT_DOMAIN_V1: &[u8] = b"sorafs.hybrid.kem.transcript:transcript-v1";
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
            HybridSuite::X25519MlKem768ChaCha20Poly1305 => {
                "x25519-mlkem768-chacha20poly1305-transcript-v1"
            }
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
            "x25519-mlkem768-chacha20poly1305-transcript-v1" => {
                Ok(Self::X25519MlKem768ChaCha20Poly1305)
            }
            _ => Err(()),
        }
    }
}

/// Errors that may occur while working with the hybrid suite helpers.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum HybridError {
    /// Invalid X25519 public key length encountered.
    #[error("invalid x25519 public key length (expected {expected}, found {found})")]
    InvalidX25519PublicKeyLength {
        /// Expected byte length.
        expected: usize,
        /// Observed byte length.
        found: usize,
    },
    /// X25519 public key is low-order and cannot contribute to the shared secret.
    #[error("x25519 public key is low-order")]
    InvalidX25519PublicKey,
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
    /// Random byte generation failed while constructing hybrid key material.
    #[error("random byte generation failed while {operation}: {message}")]
    RandomBytes {
        /// Operation that requested random bytes.
        operation: &'static str,
        /// Underlying RNG error message.
        message: String,
    },
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
        let x25519 = decode_x25519_public_key(x25519_array)?;

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
        validate_kyber_public_not_all_zero(kyber_bytes)?;

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
        validate_kyber_secret_not_all_zero(kyber_bytes)?;
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
    /// Fallibly generate a fresh key pair using the provided RNG.
    ///
    /// # Errors
    ///
    /// Returns [`HybridError`] if generated component material cannot be
    /// reconstructed through the checked hybrid secret-key parser, or if the
    /// RNG cannot provide X25519 or ML-KEM seed material.
    pub fn try_generate<R>(rng: &mut R) -> Result<Self, HybridError>
    where
        R: TryCryptoRng,
    {
        let mut x25519_bytes = Zeroizing::new([0_u8; 32]);
        fill_random(
            rng,
            "generating hybrid x25519 secret",
            x25519_bytes.as_mut(),
        )?;
        let x25519_secret = StaticSecret::from(*x25519_bytes);
        let mut kem_seed = Zeroizing::new([0_u8; 32]);
        fill_random(rng, "seeding hybrid ml-kem keypair", kem_seed.as_mut())?;
        let mut kem_rng = hedged_chacha20_rng(
            HedgedRngSeed::from_entropy(*kem_seed),
            b"iroha-crypto:hybrid:keypair",
        );
        let kem_pair = try_generate_mlkem_keypair(HYBRID_KEM_SUITE, &mut kem_rng)
            .map_err(|_| HybridError::InvalidKyberSecretKey)?;
        let x25519_secret_bytes = Zeroizing::new(x25519_secret.to_bytes());
        let secret = HybridSecretKey::from_bytes(
            x25519_secret_bytes.as_ref(),
            kem_pair.secret_key.as_slice(),
        )?;
        let public = secret.public().clone();

        Ok(Self { public, secret })
    }

    /// Generate a fresh key pair using the provided RNG.
    ///
    /// # Errors
    ///
    /// Returns [`HybridError`] if generated component material cannot be
    /// reconstructed through the checked hybrid secret-key parser, or if the
    /// RNG cannot provide X25519 or ML-KEM seed material.
    pub fn generate<R>(rng: &mut R) -> Result<Self, HybridError>
    where
        R: TryCryptoRng,
    {
        Self::try_generate(rng)
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
        let _ephemeral_public = decode_x25519_public_key(ephemeral_public_array)?;

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
    fn new(encryption_key: Zeroizing<[u8; 32]>, rekey_secret: Zeroizing<[u8; 32]>) -> Self {
        Self {
            encryption_key,
            rekey_secret,
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
        Self::new(
            Zeroizing::new(*self.encryption_key),
            Zeroizing::new(*self.rekey_secret),
        )
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
    R: TryCryptoRng,
{
    match suite {
        HybridSuite::X25519MlKem768ChaCha20Poly1305 => {
            let mut ephemeral_bytes = Zeroizing::new([0_u8; 32]);
            fill_random(
                rng,
                "generating hybrid ephemeral x25519 secret",
                ephemeral_bytes.as_mut(),
            )?;
            let ephemeral_secret = StaticSecret::from(*ephemeral_bytes);
            let ephemeral_public = X25519PublicKey::from(&ephemeral_secret);
            let shared_ecdh = ephemeral_secret.diffie_hellman(recipient.x25519());
            if shared_ecdh.as_bytes().iter().all(|&byte| byte == 0) {
                return Err(HybridError::InvalidX25519SharedSecret);
            }

            let mut kem_seed = Zeroizing::new([0_u8; 32]);
            fill_random(
                rng,
                "seeding hybrid ml-kem encapsulation",
                kem_seed.as_mut(),
            )?;
            let mut kem_rng = hedged_chacha20_rng(
                HedgedRngSeed::from_entropy(*kem_seed),
                b"iroha-crypto:hybrid:encapsulate",
            );
            let (kyber_shared, kyber_ciphertext) =
                encapsulate_mlkem(HYBRID_KEM_SUITE, recipient.kyber_bytes(), &mut kem_rng)
                    .map_err(|_| HybridError::InvalidKyberPublicKey)?;
            debug_assert_eq!(
                kyber_ciphertext.as_bytes().len(),
                HYBRID_KEM_SUITE.ciphertext_len()
            );
            let recipient_x25519 = recipient.x25519_bytes();
            let ephemeral_public_bytes = ephemeral_public.to_bytes();
            let transcript = HybridTranscript {
                recipient_x25519: &recipient_x25519,
                recipient_kyber: recipient.kyber_bytes(),
                ephemeral_x25519: &ephemeral_public_bytes,
                kyber_ciphertext: kyber_ciphertext.as_bytes(),
            };
            let derived = derive_material(
                suite,
                shared_ecdh.as_bytes(),
                kyber_shared.as_bytes(),
                transcript,
            )?;

            let ciphertext = HybridKemCiphertext {
                ephemeral_public: ephemeral_public_bytes,
                kyber_ciphertext: kyber_ciphertext.as_bytes().to_vec(),
            };

            Ok((ciphertext, derived))
        }
    }
}

fn fill_random<R: TryCryptoRng>(
    rng: &mut R,
    operation: &'static str,
    dest: &mut [u8],
) -> Result<(), HybridError> {
    rng.try_fill_bytes(dest)
        .map_err(|err| HybridError::RandomBytes {
            operation,
            message: err.to_string(),
        })
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

            let recipient_x25519 = recipient.public().x25519_bytes();
            let transcript = HybridTranscript {
                recipient_x25519: &recipient_x25519,
                recipient_kyber: recipient.public().kyber_bytes(),
                ephemeral_x25519: ciphertext.ephemeral_public(),
                kyber_ciphertext: ciphertext.kyber_ciphertext(),
            };

            derive_material(
                suite,
                shared_ecdh.as_bytes(),
                kyber_secret.as_bytes(),
                transcript,
            )
        }
    }
}

#[derive(Clone, Copy)]
struct HybridTranscript<'a> {
    recipient_x25519: &'a [u8; 32],
    recipient_kyber: &'a [u8],
    ephemeral_x25519: &'a [u8; 32],
    kyber_ciphertext: &'a [u8],
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
    transcript: HybridTranscript<'_>,
) -> Result<DerivedSecret, HybridError> {
    let transcript_components: [&[u8]; 5] = [
        SUITE_TRANSCRIPT_DOMAIN_V1,
        transcript.recipient_x25519,
        transcript.recipient_kyber,
        transcript.ephemeral_x25519,
        transcript.kyber_ciphertext,
    ];
    let mut capacity = ecdh
        .len()
        .checked_add(kyber.len())
        .ok_or(HybridError::InvalidHkdfLength)?;
    for component in transcript_components {
        capacity = capacity
            .checked_add(core::mem::size_of::<u64>())
            .and_then(|value| value.checked_add(component.len()))
            .ok_or(HybridError::InvalidHkdfLength)?;
    }

    let mut ikm = Zeroizing::new(Vec::with_capacity(capacity));
    ikm.extend_from_slice(ecdh);
    ikm.extend_from_slice(kyber);
    for component in transcript_components {
        append_transcript_component(&mut ikm, component)?;
    }
    let hkdf = Hkdf::<Sha3_256>::new(Some(suite.hkdf_salt()), ikm.as_ref());

    let mut okm = Zeroizing::new([0_u8; 64]);
    hkdf.expand(suite.hkdf_info(), okm.as_mut())
        .map_err(|_| HybridError::InvalidHkdfLength)?;

    let mut encryption_key = Zeroizing::new([0_u8; 32]);
    encryption_key.copy_from_slice(&okm[..32]);

    let mut rekey_secret = Zeroizing::new([0_u8; 32]);
    hkdf.expand(suite.rekey_info(), rekey_secret.as_mut())
        .map_err(|_| HybridError::InvalidHkdfLength)?;

    Ok(DerivedSecret::new(encryption_key, rekey_secret))
}

fn append_transcript_component(
    out: &mut Zeroizing<Vec<u8>>,
    component: &[u8],
) -> Result<(), HybridError> {
    let len = u64::try_from(component.len()).map_err(|_| HybridError::InvalidHkdfLength)?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(component);
    Ok(())
}

fn decode_x25519_public_key(bytes: [u8; 32]) -> Result<X25519PublicKey, HybridError> {
    let public_key = X25519PublicKey::from(bytes);
    if is_x25519_low_order_public_key(&public_key) {
        return Err(HybridError::InvalidX25519PublicKey);
    }
    Ok(public_key)
}

fn validate_kyber_public_not_all_zero(kyber_public: &[u8]) -> Result<(), HybridError> {
    if kyber_public.iter().all(|&byte| byte == 0) {
        return Err(HybridError::InvalidKyberPublicKey);
    }
    Ok(())
}

fn validate_kyber_secret_not_all_zero(kyber_secret: &[u8]) -> Result<(), HybridError> {
    if kyber_secret.iter().all(|&byte| byte == 0) {
        return Err(HybridError::InvalidKyberSecretKey);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng as _;
    use rand_chacha::ChaCha20Rng;
    use rand_core::{TryCryptoRng, TryRngCore};
    use zeroize::Zeroize as _;

    use super::*;

    struct FailingTryRng;

    #[derive(Debug)]
    struct FailingTryRngError;

    impl fmt::Display for FailingTryRngError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("failing hybrid RNG")
        }
    }

    impl TryRngCore for FailingTryRng {
        type Error = FailingTryRngError;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Err(FailingTryRngError)
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Err(FailingTryRngError)
        }

        fn try_fill_bytes(&mut self, _dst: &mut [u8]) -> Result<(), Self::Error> {
            Err(FailingTryRngError)
        }
    }

    impl TryCryptoRng for FailingTryRng {}

    fn set_first_mlkem_12_bit_coefficient_noncanonical(bytes: &mut [u8]) {
        bytes[0] = 0xFF;
        bytes[1] = (bytes[1] & 0xF0) | 0x0F;
    }

    fn mlkem_secret_embedded_public_range() -> core::ops::Range<usize> {
        const PUBLIC_HASH_AND_REJECTION_SEED_BYTES: usize = 64;

        let start = HYBRID_KEM_SUITE.secret_key_len()
            - HYBRID_KEM_SUITE.public_key_len()
            - PUBLIC_HASH_AND_REJECTION_SEED_BYTES;
        start..start + HYBRID_KEM_SUITE.public_key_len()
    }

    fn mlkem_secret_embedded_public_hash_range() -> core::ops::Range<usize> {
        const PUBLIC_KEY_HASH_BYTES: usize = 32;
        const PUBLIC_HASH_AND_REJECTION_SEED_BYTES: usize = 64;

        let start = HYBRID_KEM_SUITE.secret_key_len() - PUBLIC_HASH_AND_REJECTION_SEED_BYTES;
        start..start + PUBLIC_KEY_HASH_BYTES
    }

    fn mlkem_public_key_hash(public_key: &[u8]) -> [u8; 32] {
        let digest = Sha3_256::digest(public_key);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    }

    fn mlkem_secret_with_zero_embedded_public_key(secret_key: &mut [u8]) {
        let public_range = mlkem_secret_embedded_public_range();
        secret_key[public_range.clone()].fill(0);
        let public_hash = mlkem_public_key_hash(&secret_key[public_range]);
        secret_key[mlkem_secret_embedded_public_hash_range()].copy_from_slice(&public_hash);
    }

    #[test]
    fn hybrid_suite_string_is_first_release_transcript_label() {
        let suite = HybridSuite::X25519MlKem768ChaCha20Poly1305;

        assert_eq!(
            suite.to_string(),
            "x25519-mlkem768-chacha20poly1305-transcript-v1"
        );
        assert_eq!(HybridSuite::from_str(&suite.to_string()), Ok(suite));
        assert_eq!(
            HybridSuite::from_str("x25519-mlkem768-chacha20poly1305"),
            Err(())
        );
        for rejected in [
            "x25519-mlkem768-chacha20poly1305-transcript-v2",
            "x25519-mlkem768-chacha20poly1305-transcript-v1 ",
            " X25519-mlkem768-chacha20poly1305-transcript-v1",
            "x25519-mlkem768-chacha20poly1305:transcript-v1",
        ] {
            assert_eq!(HybridSuite::from_str(rejected), Err(()));
        }
    }

    #[test]
    fn derive_material_binds_public_transcript_components() {
        let ecdh = [0x11_u8; 32];
        let kyber = [0x22_u8; 32];
        let recipient_x25519 = [0x33_u8; 32];
        let recipient_kyber = vec![0x44_u8; 96];
        let ephemeral_x25519 = [0x55_u8; 32];
        let kyber_ciphertext = vec![0x66_u8; 128];

        let derive = |recipient_x25519: &[u8; 32],
                      recipient_kyber: &[u8],
                      ephemeral_x25519: &[u8; 32],
                      kyber_ciphertext: &[u8]| {
            derive_material(
                HybridSuite::X25519MlKem768ChaCha20Poly1305,
                &ecdh,
                &kyber,
                HybridTranscript {
                    recipient_x25519,
                    recipient_kyber,
                    ephemeral_x25519,
                    kyber_ciphertext,
                },
            )
            .expect("fixed HKDF inputs derive")
        };

        let baseline = derive(
            &recipient_x25519,
            &recipient_kyber,
            &ephemeral_x25519,
            &kyber_ciphertext,
        );
        let duplicate = derive(
            &recipient_x25519,
            &recipient_kyber,
            &ephemeral_x25519,
            &kyber_ciphertext,
        );
        assert_eq!(baseline.encryption_key(), duplicate.encryption_key());
        assert_eq!(baseline.rekey_secret(), duplicate.rekey_secret());

        let mut changed_recipient_x25519 = recipient_x25519;
        changed_recipient_x25519[0] ^= 0x01;
        let changed = derive(
            &changed_recipient_x25519,
            &recipient_kyber,
            &ephemeral_x25519,
            &kyber_ciphertext,
        );
        assert_ne!(baseline.encryption_key(), changed.encryption_key());

        let mut changed_recipient_kyber = recipient_kyber.clone();
        changed_recipient_kyber[0] ^= 0x01;
        let changed = derive(
            &recipient_x25519,
            &changed_recipient_kyber,
            &ephemeral_x25519,
            &kyber_ciphertext,
        );
        assert_ne!(baseline.encryption_key(), changed.encryption_key());

        let mut changed_ephemeral_x25519 = ephemeral_x25519;
        changed_ephemeral_x25519[0] ^= 0x01;
        let changed = derive(
            &recipient_x25519,
            &recipient_kyber,
            &changed_ephemeral_x25519,
            &kyber_ciphertext,
        );
        assert_ne!(baseline.encryption_key(), changed.encryption_key());

        let mut changed_ciphertext = kyber_ciphertext.clone();
        changed_ciphertext[0] ^= 0x01;
        let changed = derive(
            &recipient_x25519,
            &recipient_kyber,
            &ephemeral_x25519,
            &changed_ciphertext,
        );
        assert_ne!(baseline.encryption_key(), changed.encryption_key());
    }

    #[test]
    fn derived_secret_clone_preserves_material() {
        let ecdh = [0x10_u8; 32];
        let kyber = [0x20_u8; 32];
        let recipient_x25519 = [0x30_u8; 32];
        let recipient_kyber = vec![0x40_u8; 96];
        let ephemeral_x25519 = [0x50_u8; 32];
        let kyber_ciphertext = vec![0x60_u8; 128];

        let baseline = derive_material(
            HybridSuite::X25519MlKem768ChaCha20Poly1305,
            &ecdh,
            &kyber,
            HybridTranscript {
                recipient_x25519: &recipient_x25519,
                recipient_kyber: &recipient_kyber,
                ephemeral_x25519: &ephemeral_x25519,
                kyber_ciphertext: &kyber_ciphertext,
            },
        )
        .expect("fixed HKDF inputs derive");
        let duplicate = baseline.clone();

        assert_eq!(baseline.encryption_key(), duplicate.encryption_key());
        assert_eq!(baseline.rekey_secret(), duplicate.rekey_secret());
    }

    #[test]
    fn x25519_shared_secret_zeroizes_explicitly() {
        let secret = StaticSecret::from([0x7D; 32]);
        let peer = X25519PublicKey::from(&StaticSecret::from([0xA5; 32]));
        let mut shared = secret.diffie_hellman(&peer);

        assert!(shared.as_bytes().iter().any(|byte| *byte != 0));
        shared.zeroize();
        assert_eq!(shared.as_bytes(), &[0u8; 32]);
    }

    #[test]
    fn generated_keys_roundtrip() {
        let mut rng = ChaCha20Rng::from_seed([0x42; 32]);
        let pair = HybridKeyPair::generate(&mut rng).expect("generated hybrid keypair");

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
    fn try_generate_reports_rng_failure() {
        let mut rng = FailingTryRng;
        let err = HybridKeyPair::try_generate(&mut rng).expect_err("RNG failure must be reported");
        match err {
            HybridError::RandomBytes { operation, message } => {
                assert_eq!(operation, "generating hybrid x25519 secret");
                assert!(message.contains("failing hybrid RNG"));
            }
            other => panic!("expected RNG failure, got {other:?}"),
        }
    }

    #[test]
    fn encapsulate_reports_rng_failure() {
        let mut key_rng = ChaCha20Rng::from_seed([0x44; 32]);
        let pair = HybridKeyPair::generate(&mut key_rng).expect("generated hybrid keypair");
        let mut rng = FailingTryRng;

        let err = encapsulate(
            HybridSuite::X25519MlKem768ChaCha20Poly1305,
            pair.public(),
            &mut rng,
        )
        .expect_err("RNG failure must be reported");
        match err {
            HybridError::RandomBytes { operation, message } => {
                assert_eq!(operation, "generating hybrid ephemeral x25519 secret");
                assert!(message.contains("failing hybrid RNG"));
            }
            other => panic!("expected RNG failure, got {other:?}"),
        }
    }

    #[test]
    fn try_generate_validates_generated_key_material() {
        let mut rng = ChaCha20Rng::from_seed([0x43; 32]);
        let pair = HybridKeyPair::try_generate(&mut rng).expect("generated hybrid keypair");
        let encoded_secret = pair.secret().to_bytes();

        let decoded_secret = HybridSecretKey::from_bytes(encoded_secret.0, encoded_secret.1)
            .expect("generated secret key parses through checked constructor");

        assert_eq!(
            decoded_secret.public().kyber_bytes(),
            pair.public().kyber_bytes()
        );
        assert_eq!(
            decoded_secret.public().x25519().to_bytes(),
            pair.public().x25519().to_bytes()
        );
    }

    #[test]
    fn public_key_encoding_roundtrip() {
        let mut rng = ChaCha20Rng::from_seed([0x23; 32]);
        let pair = HybridKeyPair::generate(&mut rng).expect("generated hybrid keypair");
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
    fn public_key_decode_rejects_low_order_x25519_public_key() {
        let mut rng = ChaCha20Rng::from_seed([0x77; 32]);
        let pair = HybridKeyPair::generate(&mut rng).expect("generated hybrid keypair");
        let err = HybridPublicKey::from_bytes([0u8; 32], pair.public().kyber_bytes())
            .expect_err("low-order public key must be rejected while decoding");
        assert_eq!(err, HybridError::InvalidX25519PublicKey);
    }

    #[test]
    fn public_key_decode_rejects_noncanonical_kyber_public_key() {
        let mut rng = ChaCha20Rng::from_seed([0x7A; 32]);
        let pair = HybridKeyPair::generate(&mut rng).expect("generated hybrid keypair");
        let mut kyber_public = pair.public().kyber_bytes().to_vec();
        set_first_mlkem_12_bit_coefficient_noncanonical(&mut kyber_public);

        let err = HybridPublicKey::from_bytes(pair.public().x25519_bytes(), kyber_public)
            .expect_err("noncanonical Kyber public key must be rejected while decoding");
        assert_eq!(err, HybridError::InvalidKyberPublicKey);
    }

    #[test]
    fn public_key_decode_rejects_all_zero_kyber_public_key() {
        let mut rng = ChaCha20Rng::from_seed([0x7C; 32]);
        let pair = HybridKeyPair::generate(&mut rng).expect("generated hybrid keypair");
        let all_zero_kyber = vec![0_u8; HYBRID_KEM_SUITE.public_key_len()];

        let err = HybridPublicKey::from_bytes(pair.public().x25519_bytes(), all_zero_kyber)
            .expect_err("all-zero Kyber public key must be rejected while decoding");

        assert_eq!(err, HybridError::InvalidKyberPublicKey);
    }

    #[test]
    fn secret_key_decode_rejects_noncanonical_kyber_secret_key() {
        let mut rng = ChaCha20Rng::from_seed([0x7B; 32]);
        let pair = HybridKeyPair::generate(&mut rng).expect("generated hybrid keypair");
        let (x25519, mut kyber_secret) = pair.secret().to_bytes();
        set_first_mlkem_12_bit_coefficient_noncanonical(&mut kyber_secret);

        let err = HybridSecretKey::from_bytes(x25519, kyber_secret)
            .expect_err("noncanonical Kyber secret key must be rejected while decoding");
        assert_eq!(err, HybridError::InvalidKyberSecretKey);
    }

    #[test]
    fn secret_key_decode_rejects_all_zero_kyber_secret_key() {
        let mut rng = ChaCha20Rng::from_seed([0x7D; 32]);
        let pair = HybridKeyPair::generate(&mut rng).expect("generated hybrid keypair");
        let all_zero_kyber = vec![0_u8; HYBRID_KEM_SUITE.secret_key_len()];

        let err = HybridSecretKey::from_bytes(pair.secret().x25519().to_bytes(), all_zero_kyber)
            .expect_err("all-zero Kyber secret key must be rejected while decoding");

        assert_eq!(err, HybridError::InvalidKyberSecretKey);
    }

    #[test]
    fn secret_key_decode_rejects_all_zero_embedded_kyber_public_key() {
        let mut rng = ChaCha20Rng::from_seed([0x7E; 32]);
        let pair = HybridKeyPair::generate(&mut rng).expect("generated hybrid keypair");
        let (x25519, mut kyber_secret) = pair.secret().to_bytes();
        mlkem_secret_with_zero_embedded_public_key(&mut kyber_secret);

        let err = HybridSecretKey::from_bytes(x25519, kyber_secret)
            .expect_err("all-zero embedded Kyber public key must be rejected while decoding");

        assert_eq!(err, HybridError::InvalidKyberSecretKey);
    }

    #[test]
    fn ciphertext_decode_rejects_low_order_ephemeral_public_key() {
        let mut rng = ChaCha20Rng::from_seed([0x78; 32]);
        let pair = HybridKeyPair::generate(&mut rng).expect("generated hybrid keypair");
        let (ciphertext, _sender) = encapsulate(
            HybridSuite::X25519MlKem768ChaCha20Poly1305,
            pair.public(),
            &mut rng,
        )
        .expect("encapsulation succeeds");
        let err = HybridKemCiphertext::from_parts([0u8; 32], ciphertext.kyber_ciphertext())
            .expect_err("low-order ephemeral public key must be rejected while decoding");
        assert_eq!(err, HybridError::InvalidX25519PublicKey);
    }

    #[test]
    fn decapsulate_rejects_low_order_ephemeral_public_key() {
        let mut rng = ChaCha20Rng::from_seed([0x19; 32]);
        let pair = HybridKeyPair::generate(&mut rng).expect("generated hybrid keypair");
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
