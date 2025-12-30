use core::{fmt, str::FromStr};

use pqcrypto_kyber::{kyber512, kyber768, kyber1024};
use pqcrypto_traits::{
    Error as PqError,
    kem::{
        Ciphertext as KemCiphertext, PublicKey as KemPublicKey, SecretKey as KemSecretKey,
        SharedSecret as KemSharedSecret,
    },
};
use thiserror::Error;
use zeroize::Zeroizing;

/// Supported ML-KEM parameter sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MlKemSuite {
    /// ML-KEM-512 as specified in FIPS 203.
    MlKem512,
    /// ML-KEM-768 as specified in FIPS 203.
    MlKem768,
    /// ML-KEM-1024 as specified in FIPS 203.
    MlKem1024,
}

impl MlKemSuite {
    /// All supported parameter sets ordered by ascending security level.
    pub const ALL: [Self; 3] = [Self::MlKem512, Self::MlKem768, Self::MlKem1024];

    /// Identifier used inside `SoraNet` capability TLVs for this parameter set.
    #[must_use]
    pub const fn kem_id(self) -> u8 {
        match self {
            MlKemSuite::MlKem512 => 0,
            MlKemSuite::MlKem768 => 1,
            MlKemSuite::MlKem1024 => 2,
        }
    }

    /// Look up a suite by its capability identifier.
    #[must_use]
    pub const fn from_kem_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(MlKemSuite::MlKem512),
            1 => Some(MlKemSuite::MlKem768),
            2 => Some(MlKemSuite::MlKem1024),
            _ => None,
        }
    }

    /// Return the public key length in bytes for this parameter set.
    #[must_use]
    pub fn public_key_len(self) -> usize {
        match self {
            MlKemSuite::MlKem512 => kyber512::public_key_bytes(),
            MlKemSuite::MlKem768 => kyber768::public_key_bytes(),
            MlKemSuite::MlKem1024 => kyber1024::public_key_bytes(),
        }
    }

    /// Return the secret key length in bytes for this parameter set.
    #[must_use]
    pub fn secret_key_len(self) -> usize {
        match self {
            MlKemSuite::MlKem512 => kyber512::secret_key_bytes(),
            MlKemSuite::MlKem768 => kyber768::secret_key_bytes(),
            MlKemSuite::MlKem1024 => kyber1024::secret_key_bytes(),
        }
    }

    /// Return the ciphertext length in bytes for this parameter set.
    #[must_use]
    pub fn ciphertext_len(self) -> usize {
        match self {
            MlKemSuite::MlKem512 => kyber512::ciphertext_bytes(),
            MlKemSuite::MlKem768 => kyber768::ciphertext_bytes(),
            MlKemSuite::MlKem1024 => kyber1024::ciphertext_bytes(),
        }
    }

    /// Return the shared-secret length in bytes for this parameter set.
    #[must_use]
    pub fn shared_secret_len(self) -> usize {
        match self {
            MlKemSuite::MlKem512 => kyber512::shared_secret_bytes(),
            MlKemSuite::MlKem768 => kyber768::shared_secret_bytes(),
            MlKemSuite::MlKem1024 => kyber1024::shared_secret_bytes(),
        }
    }

    /// Return the full tuple of byte lengths for this parameter set.
    ///
    /// # Examples
    ///
    /// ```
    /// use soranet_pq::MlKemSuite;
    ///
    /// let params = MlKemSuite::MlKem512.parameters();
    /// assert_eq!(params.public_key, MlKemSuite::MlKem512.public_key_len());
    /// assert_eq!(params.secret_key, MlKemSuite::MlKem512.secret_key_len());
    /// assert_eq!(params.ciphertext, MlKemSuite::MlKem512.ciphertext_len());
    /// assert_eq!(
    ///     params.shared_secret,
    ///     MlKemSuite::MlKem512.shared_secret_len()
    /// );
    /// ```
    #[must_use]
    pub fn parameters(self) -> MlKemParameters {
        match self {
            MlKemSuite::MlKem512 => MlKemParameters {
                public_key: kyber512::public_key_bytes(),
                secret_key: kyber512::secret_key_bytes(),
                ciphertext: kyber512::ciphertext_bytes(),
                shared_secret: kyber512::shared_secret_bytes(),
            },
            MlKemSuite::MlKem768 => MlKemParameters {
                public_key: kyber768::public_key_bytes(),
                secret_key: kyber768::secret_key_bytes(),
                ciphertext: kyber768::ciphertext_bytes(),
                shared_secret: kyber768::shared_secret_bytes(),
            },
            MlKemSuite::MlKem1024 => MlKemParameters {
                public_key: kyber1024::public_key_bytes(),
                secret_key: kyber1024::secret_key_bytes(),
                ciphertext: kyber1024::ciphertext_bytes(),
                shared_secret: kyber1024::shared_secret_bytes(),
            },
        }
    }

    /// Validate a public key encoding for this suite.
    ///
    /// # Errors
    /// Returns an error when the byte string cannot be decoded.
    pub fn validate_public_key(self, bytes: &[u8]) -> Result<(), MlKemError> {
        match self {
            MlKemSuite::MlKem512 => kyber512::PublicKey::from_bytes(bytes)
                .map(|_| ())
                .map_err(|err| MlKemError::bad_encoding("ML-KEM-512 public key", err)),
            MlKemSuite::MlKem768 => kyber768::PublicKey::from_bytes(bytes)
                .map(|_| ())
                .map_err(|err| MlKemError::bad_encoding("ML-KEM-768 public key", err)),
            MlKemSuite::MlKem1024 => kyber1024::PublicKey::from_bytes(bytes)
                .map(|_| ())
                .map_err(|err| MlKemError::bad_encoding("ML-KEM-1024 public key", err)),
        }
    }

    /// Validate a secret key encoding for this suite.
    ///
    /// # Errors
    /// Returns an error when the byte string cannot be decoded.
    pub fn validate_secret_key(self, bytes: &[u8]) -> Result<(), MlKemError> {
        match self {
            MlKemSuite::MlKem512 => kyber512::SecretKey::from_bytes(bytes)
                .map(|_| ())
                .map_err(|err| MlKemError::bad_encoding("ML-KEM-512 secret key", err)),
            MlKemSuite::MlKem768 => kyber768::SecretKey::from_bytes(bytes)
                .map(|_| ())
                .map_err(|err| MlKemError::bad_encoding("ML-KEM-768 secret key", err)),
            MlKemSuite::MlKem1024 => kyber1024::SecretKey::from_bytes(bytes)
                .map(|_| ())
                .map_err(|err| MlKemError::bad_encoding("ML-KEM-1024 secret key", err)),
        }
    }

    /// Validate a ciphertext encoding for this suite.
    ///
    /// # Errors
    /// Returns an error when the byte string cannot be decoded.
    pub fn validate_ciphertext(self, bytes: &[u8]) -> Result<(), MlKemError> {
        match self {
            MlKemSuite::MlKem512 => kyber512::Ciphertext::from_bytes(bytes)
                .map_err(|err| MlKemError::bad_encoding("ML-KEM-512 ciphertext", err))
                .map(|_| ()),
            MlKemSuite::MlKem768 => kyber768::Ciphertext::from_bytes(bytes)
                .map_err(|err| MlKemError::bad_encoding("ML-KEM-768 ciphertext", err))
                .map(|_| ()),
            MlKemSuite::MlKem1024 => kyber1024::Ciphertext::from_bytes(bytes)
                .map_err(|err| MlKemError::bad_encoding("ML-KEM-1024 ciphertext", err))
                .map(|_| ()),
        }
    }
}

/// Errors produced when parsing an [`MlKemSuite`] from text.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("unknown ML-KEM suite '{0}'")]
pub struct SuiteParseError(pub String);

impl fmt::Display for MlKemSuite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            MlKemSuite::MlKem512 => "mlkem512",
            MlKemSuite::MlKem768 => "mlkem768",
            MlKemSuite::MlKem1024 => "mlkem1024",
        })
    }
}

impl FromStr for MlKemSuite {
    type Err = SuiteParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_ascii_lowercase().as_str() {
            "mlkem512" | "kyber512" => Ok(MlKemSuite::MlKem512),
            "mlkem768" | "kyber768" => Ok(MlKemSuite::MlKem768),
            "mlkem1024" | "kyber1024" => Ok(MlKemSuite::MlKem1024),
            _ => Err(SuiteParseError(input.to_string())),
        }
    }
}

/// Byte lengths for an ML-KEM suite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MlKemParameters {
    /// Public key size in bytes.
    pub public_key: usize,
    /// Secret key size in bytes.
    pub secret_key: usize,
    /// Ciphertext size in bytes.
    pub ciphertext: usize,
    /// Shared secret size in bytes.
    pub shared_secret: usize,
}

impl From<MlKemSuite> for MlKemParameters {
    fn from(suite: MlKemSuite) -> Self {
        suite.parameters()
    }
}

/// Detailed metadata for an ML-KEM suite as specified in FIPS 203.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MlKemMetadata {
    /// Suite identifier (`MlKemSuite`) this metadata describes.
    pub suite: MlKemSuite,
    /// Human-readable ML-KEM suite name.
    pub name: &'static str,
    /// Canonical `SoraNet` capability identifier (`snnet.pqkem` `kem_id`).
    pub kem_id: u8,
    /// NIST security strength category (1, 3, or 5).
    pub security_category: u8,
    /// Target symmetric security in bits (e.g., 128, 192, 256).
    pub symmetric_security_bits: u16,
    /// Degree of the module lattice polynomial (`n`).
    pub module_degree: u16,
    /// Rank of the module (`k` parameter from FIPS 203).
    pub module_rank: u8,
    /// Modulus `q` for the ring `Z_q[x]/(x^n + 1)`.
    pub modulus_q: u16,
    /// Noise parameter `η₁` used when sampling secret polynomials.
    pub eta1: u8,
    /// Noise parameter `η₂` used during reconciliation.
    pub eta2: u8,
    /// Compression parameter `d_u` applied to the public key.
    pub du: u8,
    /// Compression parameter `d_v` applied to the ciphertext.
    pub dv: u8,
    /// Byte lengths for the suite.
    pub parameters: MlKemParameters,
}

impl MlKemSuite {
    /// Return the metadata record describing this suite.
    #[must_use]
    pub const fn metadata(self) -> MlKemMetadata {
        match self {
            MlKemSuite::MlKem512 => MlKemMetadata {
                suite: MlKemSuite::MlKem512,
                name: "ML-KEM-512",
                kem_id: 0,
                security_category: 1,
                symmetric_security_bits: 128,
                module_degree: 256,
                module_rank: 2,
                modulus_q: 3329,
                eta1: 3,
                eta2: 2,
                du: 10,
                dv: 4,
                parameters: MlKemParameters {
                    public_key: 800,
                    secret_key: 1632,
                    ciphertext: 768,
                    shared_secret: 32,
                },
            },
            MlKemSuite::MlKem768 => MlKemMetadata {
                suite: MlKemSuite::MlKem768,
                name: "ML-KEM-768",
                kem_id: 1,
                security_category: 3,
                symmetric_security_bits: 192,
                module_degree: 256,
                module_rank: 3,
                modulus_q: 3329,
                eta1: 2,
                eta2: 2,
                du: 10,
                dv: 4,
                parameters: MlKemParameters {
                    public_key: 1184,
                    secret_key: 2400,
                    ciphertext: 1088,
                    shared_secret: 32,
                },
            },
            MlKemSuite::MlKem1024 => MlKemMetadata {
                suite: MlKemSuite::MlKem1024,
                name: "ML-KEM-1024",
                kem_id: 2,
                security_category: 5,
                symmetric_security_bits: 256,
                module_degree: 256,
                module_rank: 4,
                modulus_q: 3329,
                eta1: 2,
                eta2: 2,
                du: 11,
                dv: 5,
                parameters: MlKemParameters {
                    public_key: 1568,
                    secret_key: 3168,
                    ciphertext: 1568,
                    shared_secret: 32,
                },
            },
        }
    }
}

/// Wrapper around an ML-KEM keypair (public + secret).
#[derive(Debug)]
pub struct MlKemKeyPair {
    /// Public key bytes.
    pub public_key: Vec<u8>,
    /// Secret key bytes, zeroized on drop.
    pub secret_key: Zeroizing<Vec<u8>>,
}

impl MlKemKeyPair {
    /// Return the public key as raw bytes.
    #[must_use]
    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    /// Return the secret key as raw bytes.
    #[must_use]
    pub fn secret_key(&self) -> &[u8] {
        &self.secret_key
    }
}

/// Encapsulated ML-KEM ciphertext.
#[derive(Debug, Clone)]
pub struct MlKemCiphertext {
    bytes: Vec<u8>,
}

impl MlKemCiphertext {
    /// Construct from raw ciphertext bytes.
    fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Access the ciphertext bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Shared secret output of an ML-KEM operation.
#[derive(Debug, Clone)]
pub struct MlKemSharedSecret {
    bytes: Zeroizing<Vec<u8>>,
}

impl MlKemSharedSecret {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Zeroizing::new(bytes),
        }
    }

    /// Access the shared secret bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Errors that can arise while working with ML-KEM wrappers.
#[derive(Clone, Copy, Debug, Error)]
pub enum MlKemError {
    /// Input byte string had an unexpected length.
    #[error("invalid {kind} encoding: {source}")]
    BadEncoding {
        /// Identifier of the field that failed to decode.
        kind: &'static str,
        /// Original `PQClean` error.
        #[source]
        source: PqError,
    },
}

impl MlKemError {
    fn bad_encoding(kind: &'static str, source: PqError) -> Self {
        MlKemError::BadEncoding { kind, source }
    }
}

/// Generate an ML-KEM keypair for the given parameter set.
pub fn generate_mlkem_keypair(suite: MlKemSuite) -> MlKemKeyPair {
    match suite {
        MlKemSuite::MlKem512 => {
            let (pk, sk) = kyber512::keypair();
            MlKemKeyPair {
                public_key: pk.as_bytes().to_vec(),
                secret_key: Zeroizing::new(sk.as_bytes().to_vec()),
            }
        }
        MlKemSuite::MlKem768 => {
            let (pk, sk) = kyber768::keypair();
            MlKemKeyPair {
                public_key: pk.as_bytes().to_vec(),
                secret_key: Zeroizing::new(sk.as_bytes().to_vec()),
            }
        }
        MlKemSuite::MlKem1024 => {
            let (pk, sk) = kyber1024::keypair();
            MlKemKeyPair {
                public_key: pk.as_bytes().to_vec(),
                secret_key: Zeroizing::new(sk.as_bytes().to_vec()),
            }
        }
    }
}

/// Encapsulate against a provided public key.
///
/// # Errors
/// Returns an error when the public key encoding is invalid.
pub fn encapsulate_mlkem(
    suite: MlKemSuite,
    public_key: &[u8],
) -> Result<(MlKemSharedSecret, MlKemCiphertext), MlKemError> {
    match suite {
        MlKemSuite::MlKem512 => {
            let pk = kyber512::PublicKey::from_bytes(public_key)
                .map_err(|err| MlKemError::bad_encoding("ML-KEM-512 public key", err))?;
            let (shared, ct) = kyber512::encapsulate(&pk);
            Ok((
                MlKemSharedSecret::new(shared.as_bytes().to_vec()),
                MlKemCiphertext::new(ct.as_bytes().to_vec()),
            ))
        }
        MlKemSuite::MlKem768 => {
            let pk = kyber768::PublicKey::from_bytes(public_key)
                .map_err(|err| MlKemError::bad_encoding("ML-KEM-768 public key", err))?;
            let (shared, ct) = kyber768::encapsulate(&pk);
            Ok((
                MlKemSharedSecret::new(shared.as_bytes().to_vec()),
                MlKemCiphertext::new(ct.as_bytes().to_vec()),
            ))
        }
        MlKemSuite::MlKem1024 => {
            let pk = kyber1024::PublicKey::from_bytes(public_key)
                .map_err(|err| MlKemError::bad_encoding("ML-KEM-1024 public key", err))?;
            let (shared, ct) = kyber1024::encapsulate(&pk);
            Ok((
                MlKemSharedSecret::new(shared.as_bytes().to_vec()),
                MlKemCiphertext::new(ct.as_bytes().to_vec()),
            ))
        }
    }
}

/// Decapsulate a ciphertext with the provided secret key.
///
/// # Errors
/// Returns an error when the secret key or ciphertext encoding is invalid.
pub fn decapsulate_mlkem(
    suite: MlKemSuite,
    secret_key: &[u8],
    ciphertext: &[u8],
) -> Result<MlKemSharedSecret, MlKemError> {
    match suite {
        MlKemSuite::MlKem512 => {
            let sk = kyber512::SecretKey::from_bytes(secret_key)
                .map_err(|err| MlKemError::bad_encoding("ML-KEM-512 secret key", err))?;
            let ct = kyber512::Ciphertext::from_bytes(ciphertext)
                .map_err(|err| MlKemError::bad_encoding("ML-KEM-512 ciphertext", err))?;
            let shared = kyber512::decapsulate(&ct, &sk);
            Ok(MlKemSharedSecret::new(shared.as_bytes().to_vec()))
        }
        MlKemSuite::MlKem768 => {
            let sk = kyber768::SecretKey::from_bytes(secret_key)
                .map_err(|err| MlKemError::bad_encoding("ML-KEM-768 secret key", err))?;
            let ct = kyber768::Ciphertext::from_bytes(ciphertext)
                .map_err(|err| MlKemError::bad_encoding("ML-KEM-768 ciphertext", err))?;
            let shared = kyber768::decapsulate(&ct, &sk);
            Ok(MlKemSharedSecret::new(shared.as_bytes().to_vec()))
        }
        MlKemSuite::MlKem1024 => {
            let sk = kyber1024::SecretKey::from_bytes(secret_key)
                .map_err(|err| MlKemError::bad_encoding("ML-KEM-1024 secret key", err))?;
            let ct = kyber1024::Ciphertext::from_bytes(ciphertext)
                .map_err(|err| MlKemError::bad_encoding("ML-KEM-1024 ciphertext", err))?;
            let shared = kyber1024::decapsulate(&ct, &sk);
            Ok(MlKemSharedSecret::new(shared.as_bytes().to_vec()))
        }
    }
}

/// Return parameter lengths for the given ML-KEM suite.
#[must_use]
pub fn mlkem_parameters(suite: MlKemSuite) -> MlKemParameters {
    suite.metadata().parameters
}

/// Return the metadata record for the provided ML-KEM suite.
#[must_use]
pub fn mlkem_metadata(suite: MlKemSuite) -> MlKemMetadata {
    suite.metadata()
}

/// Validate the encoding of an ML-KEM public key.
///
/// # Errors
/// Returns an error when the public key encoding is invalid.
pub fn validate_mlkem_public_key(suite: MlKemSuite, bytes: &[u8]) -> Result<(), MlKemError> {
    match suite {
        MlKemSuite::MlKem512 => kyber512::PublicKey::from_bytes(bytes)
            .map(|_| ())
            .map_err(|err| MlKemError::bad_encoding("ML-KEM-512 public key", err)),
        MlKemSuite::MlKem768 => kyber768::PublicKey::from_bytes(bytes)
            .map(|_| ())
            .map_err(|err| MlKemError::bad_encoding("ML-KEM-768 public key", err)),
        MlKemSuite::MlKem1024 => kyber1024::PublicKey::from_bytes(bytes)
            .map(|_| ())
            .map_err(|err| MlKemError::bad_encoding("ML-KEM-1024 public key", err)),
    }
}

/// Validate the encoding of an ML-KEM secret key.
///
/// # Errors
/// Returns an error when the secret key encoding is invalid.
pub fn validate_mlkem_secret_key(suite: MlKemSuite, bytes: &[u8]) -> Result<(), MlKemError> {
    match suite {
        MlKemSuite::MlKem512 => kyber512::SecretKey::from_bytes(bytes)
            .map(|_| ())
            .map_err(|err| MlKemError::bad_encoding("ML-KEM-512 secret key", err)),
        MlKemSuite::MlKem768 => kyber768::SecretKey::from_bytes(bytes)
            .map(|_| ())
            .map_err(|err| MlKemError::bad_encoding("ML-KEM-768 secret key", err)),
        MlKemSuite::MlKem1024 => kyber1024::SecretKey::from_bytes(bytes)
            .map(|_| ())
            .map_err(|err| MlKemError::bad_encoding("ML-KEM-1024 secret key", err)),
    }
}

/// Validate the encoding of an ML-KEM ciphertext.
///
/// # Errors
/// Returns an error when the ciphertext encoding is invalid.
pub fn validate_mlkem_ciphertext(suite: MlKemSuite, bytes: &[u8]) -> Result<(), MlKemError> {
    match suite {
        MlKemSuite::MlKem512 => kyber512::Ciphertext::from_bytes(bytes)
            .map(|_| ())
            .map_err(|err| MlKemError::bad_encoding("ML-KEM-512 ciphertext", err)),
        MlKemSuite::MlKem768 => kyber768::Ciphertext::from_bytes(bytes)
            .map(|_| ())
            .map_err(|err| MlKemError::bad_encoding("ML-KEM-768 ciphertext", err)),
        MlKemSuite::MlKem1024 => kyber1024::Ciphertext::from_bytes(bytes)
            .map(|_| ())
            .map_err(|err| MlKemError::bad_encoding("ML-KEM-1024 ciphertext", err)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(suite: MlKemSuite) {
        let keys = generate_mlkem_keypair(suite);
        let (shared_a, ct) = encapsulate_mlkem(suite, keys.public_key()).unwrap();
        let shared_b = decapsulate_mlkem(suite, keys.secret_key(), ct.as_bytes()).unwrap();
        assert_eq!(shared_a.as_bytes(), shared_b.as_bytes());
    }

    #[test]
    fn roundtrip_512() {
        roundtrip(MlKemSuite::MlKem512);
    }

    #[test]
    fn roundtrip_768() {
        roundtrip(MlKemSuite::MlKem768);
    }

    #[test]
    fn roundtrip_1024() {
        roundtrip(MlKemSuite::MlKem1024);
    }

    #[test]
    fn invalid_public_key_length() {
        let err = encapsulate_mlkem(MlKemSuite::MlKem512, &[0u8; 8]).unwrap_err();
        match err {
            MlKemError::BadEncoding { kind, .. } => assert!(kind.contains("public key")),
        }
    }

    #[test]
    fn mlkem_parameters_align_with_bindings() {
        let params = mlkem_parameters(MlKemSuite::MlKem768);
        assert_eq!(params.public_key, kyber768::public_key_bytes());
        assert_eq!(params.secret_key, kyber768::secret_key_bytes());
        assert_eq!(params.ciphertext, kyber768::ciphertext_bytes());
        assert_eq!(params.shared_secret, kyber768::shared_secret_bytes());
    }

    #[test]
    fn validation_rejects_short_inputs() {
        let res = validate_mlkem_public_key(MlKemSuite::MlKem1024, &[0u8; 16]);
        assert!(res.is_err());
    }

    #[test]
    fn suite_parameters_cover_all_lengths() {
        for suite in MlKemSuite::ALL {
            let via_method = suite.parameters();
            let via_fn = mlkem_parameters(suite);
            assert_eq!(via_method, via_fn);
            assert_eq!(via_method.public_key, suite.public_key_len());
            assert_eq!(via_method.secret_key, suite.secret_key_len());
            assert_eq!(via_method.ciphertext, suite.ciphertext_len());
            assert_eq!(via_method.shared_secret, suite.shared_secret_len());
        }
    }

    #[test]
    fn suite_into_parameters_matches_method() {
        let params_from_into: MlKemParameters = MlKemSuite::MlKem768.into();
        let params_from_method = MlKemSuite::MlKem768.parameters();
        assert_eq!(params_from_into, params_from_method);
    }

    #[test]
    fn metadata_roundtrips_kem_id() {
        for suite in MlKemSuite::ALL {
            let metadata = suite.metadata();
            assert_eq!(metadata.suite, suite);
            assert_eq!(suite.kem_id(), metadata.kem_id);
            let recovered =
                MlKemSuite::from_kem_id(metadata.kem_id).expect("supported kem identifier");
            assert_eq!(recovered, suite);
        }
        assert!(MlKemSuite::from_kem_id(0xFF).is_none());
    }

    #[test]
    fn metadata_matches_bindings() {
        for suite in MlKemSuite::ALL {
            let metadata = suite.metadata();
            let params = metadata.parameters;
            assert_eq!(params, mlkem_parameters(suite));
            assert_eq!(params, suite.parameters());

            match suite {
                MlKemSuite::MlKem512 => {
                    assert_eq!(metadata.name, "ML-KEM-512");
                    assert_eq!(metadata.security_category, 1);
                    assert_eq!(metadata.symmetric_security_bits, 128);
                    assert_eq!(metadata.module_rank, 2);
                    assert_eq!(metadata.du, 10);
                    assert_eq!(metadata.dv, 4);
                }
                MlKemSuite::MlKem768 => {
                    assert_eq!(metadata.name, "ML-KEM-768");
                    assert_eq!(metadata.security_category, 3);
                    assert_eq!(metadata.symmetric_security_bits, 192);
                    assert_eq!(metadata.module_rank, 3);
                    assert_eq!(metadata.du, 10);
                    assert_eq!(metadata.dv, 4);
                }
                MlKemSuite::MlKem1024 => {
                    assert_eq!(metadata.name, "ML-KEM-1024");
                    assert_eq!(metadata.security_category, 5);
                    assert_eq!(metadata.symmetric_security_bits, 256);
                    assert_eq!(metadata.module_rank, 4);
                    assert_eq!(metadata.du, 11);
                    assert_eq!(metadata.dv, 5);
                }
            }

            assert_eq!(metadata.module_degree, 256);
            assert_eq!(metadata.modulus_q, 3329);
            assert_eq!(metadata.eta2, 2);
            assert_eq!(metadata.parameters.shared_secret, 32);
        }
    }

    #[test]
    fn suite_parsing_accepts_common_aliases() {
        assert_eq!(
            MlKemSuite::from_str("mlkem512").unwrap(),
            MlKemSuite::MlKem512
        );
        assert_eq!(
            MlKemSuite::from_str("KYBER768").unwrap(),
            MlKemSuite::MlKem768
        );
        assert_eq!(
            MlKemSuite::from_str("MlKeM1024").unwrap(),
            MlKemSuite::MlKem1024
        );
        let err = MlKemSuite::from_str("unknown-suite").unwrap_err();
        assert_eq!(err, SuiteParseError("unknown-suite".to_string()));
    }
}
