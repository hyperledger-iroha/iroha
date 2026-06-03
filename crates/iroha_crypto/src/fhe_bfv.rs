//! Deterministic BFV-shaped baseline for homomorphic scalar evaluation.
//!
//! This module implements an exact plaintext-lift evaluator over the negacyclic
//! ring `Z_q[x] / (x^n + 1)` with `q` divisible by the plaintext modulus `t`.
//! The first-release RAM-LFE profile uses zero error terms so ciphertext
//! Add/Multiply/SelectEqZero remain exact through the published hidden-program
//! depth while evaluators stay secret-key free. The API surface includes:
//! - seeded key generation,
//! - public-key encryption / secret-key decryption,
//! - ciphertext addition,
//! - ciphertext-by-plaintext multiplication,
//! - ciphertext-by-ciphertext multiplication with relinearization,
//! - and a compact affine-circuit evaluator over scalar ciphertext inputs.
//!
//! TODO: Replace the exact zero-error plaintext-lift profile with the planned
//! BFV-RNS modulus-chain, bounded-noise, and bootstrapping engine before calling
//! this a security-complete BFV implementation.
//!
//! The implementation keeps a deterministic scalar fallback for every path.
//! When the `bfv-accel` feature is enabled, polynomial multiplication switches
//! to an exact CRT-NTT backend over NTT-friendly helper primes and then folds
//! the linear product back into the negacyclic BFV ring. This keeps observable
//! outputs identical across hardware while substantially reducing the cost of
//! ciphertext multiplication for the parameter sets used by identifier lookup.

use std::{fmt, string::String, vec::Vec};

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::derive::{JsonDeserialize, JsonSerialize};
use rand::{Rng as _, SeedableRng as _};
use rand_chacha::ChaCha20Rng;
use thiserror::Error;

use crate::Hash;

const KEYGEN_DOMAIN: &[u8] = b"iroha.crypto.fhe.bfv.keygen.v1";
const ENCRYPT_DOMAIN: &[u8] = b"iroha.crypto.fhe.bfv.encrypt.v1";
const IDENTIFIER_KEYGEN_DOMAIN: &[u8] = b"iroha.crypto.fhe.bfv.identifier.keygen.v1";
const IDENTIFIER_SLOT_ENCRYPT_DOMAIN: &[u8] = b"iroha.crypto.fhe.bfv.identifier.slot.v1";
const BFV_PARAMETER_DIGEST_DOMAIN: &[u8] = b"iroha.crypto.fhe.bfv.parameter_digest.v1";
const BFV_EVALUATION_KEY_DIGEST_DOMAIN: &[u8] = b"iroha.crypto.fhe.bfv.eval_key_digest.v1";
const BFV_BOOTSTRAP_KEY_ID_MAX_BYTES: usize = 128;
const BFV_EVALUATION_KEY_MAX_ROTATION_KEYS: usize = 64;

/// Registered RAM-LFE BFV plaintext modulus.
///
/// RAM-LFE byte predicates evaluate `eq0(x) = 1 - x^256`; this requires a
/// prime plaintext field that contains every byte value as a field element.
pub const RAM_LFE_BFV_PLAINTEXT_MODULUS: u64 = 257;

/// Registered RAM-LFE BFV ciphertext modulus.
pub const RAM_LFE_BFV_CIPHERTEXT_MODULUS: u64 = RAM_LFE_BFV_PLAINTEXT_MODULUS * (1_u64 << 48);

type Polynomial = Vec<u64>;

#[cfg(feature = "bfv-accel")]
#[derive(Clone, Copy, Debug)]
struct NttPrime {
    modulus: u64,
    primitive_root: u64,
    max_power_of_two: u32,
}

#[cfg(feature = "bfv-accel")]
const CRT_NTT_PRIMES: [NttPrime; 4] = [
    NttPrime {
        modulus: 4_293_918_721,
        primitive_root: 19,
        max_power_of_two: 20,
    },
    NttPrime {
        modulus: 4_292_804_609,
        primitive_root: 3,
        max_power_of_two: 16,
    },
    NttPrime {
        modulus: 4_292_149_249,
        primitive_root: 14,
        max_power_of_two: 16,
    },
    NttPrime {
        modulus: 4_292_018_177,
        primitive_root: 5,
        max_power_of_two: 16,
    },
];

/// Polynomial multiplication backend selected for BFV ring products.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BfvConvolutionBackend {
    /// Textbook schoolbook negacyclic multiplication.
    ScalarSchoolbook,
    /// Exact CRT-NTT negacyclic multiplication.
    CrtNtt,
}

/// BFV parameter set.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct BfvParameters {
    /// Ring degree `n` for `Z_q[x] / (x^n + 1)`. Must be a power of two.
    pub polynomial_degree: u16,
    /// Ciphertext modulus `q`.
    pub ciphertext_modulus: u64,
    /// Plaintext modulus `t`.
    pub plaintext_modulus: u64,
    /// Base-2 logarithm of the relinearization decomposition base.
    pub decomposition_base_log: u8,
}

impl BfvParameters {
    /// Validate the parameter set.
    ///
    /// # Errors
    /// Returns [`BfvError`] when the parameter set is internally inconsistent.
    pub fn validate(&self) -> Result<(), BfvError> {
        let n = usize::from(self.polynomial_degree);
        if n < 2 || !n.is_power_of_two() {
            return Err(BfvError::InvalidParameters(
                "polynomial_degree must be a power of two and at least 2".to_owned(),
            ));
        }
        if self.plaintext_modulus < 2 {
            return Err(BfvError::InvalidParameters(
                "plaintext_modulus must be at least 2".to_owned(),
            ));
        }
        if self.ciphertext_modulus <= self.plaintext_modulus {
            return Err(BfvError::InvalidParameters(
                "ciphertext_modulus must be greater than plaintext_modulus".to_owned(),
            ));
        }
        if !self
            .ciphertext_modulus
            .is_multiple_of(self.plaintext_modulus)
        {
            return Err(BfvError::InvalidParameters(
                "ciphertext_modulus must be divisible by plaintext_modulus".to_owned(),
            ));
        }
        if self.decomposition_base_log == 0 || self.decomposition_base_log > 16 {
            return Err(BfvError::InvalidParameters(
                "decomposition_base_log must be within 1..=16".to_owned(),
            ));
        }
        let max_raw_coefficient = u128::from(self.polynomial_degree)
            .saturating_mul(u128::from(self.ciphertext_modulus))
            .saturating_mul(u128::from(self.ciphertext_modulus));
        let max_scaled_coefficient =
            max_raw_coefficient.saturating_mul(u128::from(self.plaintext_modulus));
        if max_scaled_coefficient > i128::MAX as u128 {
            return Err(BfvError::InvalidParameters(
                "parameter set exceeds the deterministic BFV exact-arithmetic overflow bounds"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    /// Report the polynomial-convolution backend used for BFV ring products.
    #[must_use]
    pub fn convolution_backend(&self) -> BfvConvolutionBackend {
        #[cfg(not(feature = "bfv-accel"))]
        let _ = self;

        #[cfg(feature = "bfv-accel")]
        {
            let Some(convolution_len) = self.degree().checked_mul(2) else {
                return BfvConvolutionBackend::ScalarSchoolbook;
            };
            if convolution_len == 0 {
                return BfvConvolutionBackend::ScalarSchoolbook;
            }
            let required_log = convolution_len.ilog2();
            if convolution_len.is_power_of_two()
                && CRT_NTT_PRIMES
                    .iter()
                    .all(|prime| prime.max_power_of_two >= required_log)
            {
                return BfvConvolutionBackend::CrtNtt;
            }
        }
        BfvConvolutionBackend::ScalarSchoolbook
    }

    fn degree(&self) -> usize {
        usize::from(self.polynomial_degree)
    }

    fn decomposition_base(&self) -> u64 {
        1_u64 << self.decomposition_base_log
    }

    fn decomposition_digits(&self) -> usize {
        let mut digits = 0_usize;
        let mut covered = 1_u128;
        let base = u128::from(self.decomposition_base());
        let modulus = u128::from(self.ciphertext_modulus);
        while covered < modulus {
            covered = covered.saturating_mul(base);
            digits = digits.saturating_add(1);
        }
        digits.max(1)
    }
}

/// BFV secret key.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct BfvSecretKey {
    /// Ternary secret polynomial in `R_q`.
    pub s: Vec<u64>,
}

/// BFV public key.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct BfvPublicKey {
    /// First public-key component.
    pub b: Vec<u64>,
    /// Second public-key component.
    pub a: Vec<u64>,
}

/// One base-decomposition entry of the relinearization key.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct BfvRelinearizationKeyEntry {
    /// First evaluation-key component.
    pub b: Vec<u64>,
    /// Second evaluation-key component.
    pub a: Vec<u64>,
}

/// Relinearization key for reducing quadratic ciphertexts back to size two.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct BfvRelinearizationKey {
    /// Decomposition entries, ordered from least- to most-significant digit.
    pub entries: Vec<BfvRelinearizationKeyEntry>,
}

/// Public rotation key admitted for deterministic ciphertext-slot rotations.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct BfvRotationKey {
    /// Positive left-rotation step supported by this key.
    pub rotation_steps: u32,
    /// Encryption of zero added to each moved slot during rotation refresh.
    ///
    /// Soracloud `RotateLeft` is defined over the outer identifier ciphertext
    /// envelope, where each logical byte slot is an independent scalar BFV
    /// ciphertext. The key holder prepares this mask with the BFV public key so
    /// evaluators can rotate and refresh slots without holding the secret key.
    pub zero_refresh: BfvCiphertext,
}

/// Public bootstrap key admitted for deterministic ciphertext refresh.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct BfvBootstrapKey {
    /// Stable bootstrap-key identifier.
    pub key_id: String,
    /// Encryption of zero added during refresh.
    ///
    /// The key holder prepares this ciphertext with the BFV public key for the
    /// registered parameter set. Evaluators can refresh/re-randomize a
    /// ciphertext by homomorphically adding this mask without holding the secret
    /// key or observing the plaintext.
    pub zero_refresh: BfvCiphertext,
}

/// Evaluation keys required by public BFV evaluators.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct BfvEvaluationKeyBundle {
    /// Relinearization key used after ciphertext-ciphertext multiplication.
    pub relinearization_key: BfvRelinearizationKey,
    /// Rotation keys admitted for slot rotation.
    #[norito(default)]
    pub rotation_keys: Vec<BfvRotationKey>,
    /// Optional bootstrap key used to refresh ciphertexts.
    #[norito(default)]
    pub bootstrap_key: Option<BfvBootstrapKey>,
}

impl BfvEvaluationKeyBundle {
    /// Validate evaluation-key shapes against BFV parameters.
    ///
    /// # Errors
    /// Returns [`BfvError`] when key material is malformed or duplicated.
    pub fn validate(&self, params: &BfvParameters) -> Result<(), BfvError> {
        params.validate()?;
        validate_relinearization_key(params, &self.relinearization_key)?;
        if self.rotation_keys.len() > BFV_EVALUATION_KEY_MAX_ROTATION_KEYS {
            return Err(BfvError::InvalidParameters(format!(
                "evaluation-key bundle supports at most {BFV_EVALUATION_KEY_MAX_ROTATION_KEYS} rotation keys"
            )));
        }
        let mut seen_rotations = std::collections::BTreeSet::new();
        for key in &self.rotation_keys {
            if key.rotation_steps == 0 {
                return Err(BfvError::InvalidParameters(
                    "rotation key steps must be greater than zero".to_owned(),
                ));
            }
            if !seen_rotations.insert(key.rotation_steps) {
                return Err(BfvError::InvalidParameters(format!(
                    "duplicate rotation key for {} steps",
                    key.rotation_steps
                )));
            }
            validate_ciphertext(params, &key.zero_refresh)?;
        }
        if let Some(bootstrap_key) = self.bootstrap_key.as_ref() {
            validate_bootstrap_key_id(&bootstrap_key.key_id)?;
            validate_ciphertext(params, &bootstrap_key.zero_refresh)?;
        }
        Ok(())
    }

    /// Return a stable digest over the evaluation-key bundle.
    ///
    /// # Errors
    /// Returns [`BfvError`] when validation or canonical encoding fails.
    pub fn digest(&self, params: &BfvParameters) -> Result<Hash, BfvError> {
        self.validate(params)?;
        let bytes = norito::to_bytes(self).map_err(|err| {
            BfvError::InvalidParameters(format!("evaluation key encoding failed: {err}"))
        })?;
        Ok(Hash::new(
            [BFV_EVALUATION_KEY_DIGEST_DOMAIN, bytes.as_slice()].concat(),
        ))
    }
}

/// BFV ciphertext with two components.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct BfvCiphertext {
    /// First ciphertext polynomial.
    pub c0: Vec<u64>,
    /// Second ciphertext polynomial.
    pub c1: Vec<u64>,
}

/// Public BFV parameters published to clients for encrypted identifier input.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct BfvIdentifierPublicParameters {
    /// Underlying BFV parameter set.
    pub parameters: BfvParameters,
    /// Public key used to encrypt identifier input.
    pub public_key: BfvPublicKey,
    /// Maximum number of raw UTF-8 input bytes accepted by the envelope.
    pub max_input_bytes: u16,
}

/// BFV ciphertext envelope for identifier input.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct BfvIdentifierCiphertext {
    /// Scalar ciphertext slots: slot 0 is the byte length, followed by one slot per byte.
    pub slots: Vec<BfvCiphertext>,
}

impl BfvIdentifierPublicParameters {
    /// Validate the public parameters and envelope capacity.
    ///
    /// # Errors
    /// Returns [`BfvError`] when the envelope is internally inconsistent or
    /// does not use a registered production BFV parameter profile.
    pub fn validate(&self) -> Result<(), BfvError> {
        validate_registered_bfv_parameters(&self.parameters)?;
        validate_public_key(&self.parameters, &self.public_key)?;
        if self.max_input_bytes == 0 {
            return Err(BfvError::InvalidParameters(
                "max_input_bytes must be at least 1".to_owned(),
            ));
        }
        if u64::from(self.max_input_bytes) >= self.parameters.plaintext_modulus {
            return Err(BfvError::InvalidParameters(
                "max_input_bytes must fit into one plaintext slot".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Return the registered BFV parameter set used by RAM-LFE byte-slot programs.
#[must_use]
pub const fn ram_lfe_bfv_parameters_v1() -> BfvParameters {
    BfvParameters {
        polynomial_degree: 64,
        ciphertext_modulus: RAM_LFE_BFV_CIPHERTEXT_MODULUS,
        plaintext_modulus: RAM_LFE_BFV_PLAINTEXT_MODULUS,
        decomposition_base_log: 12,
    }
}

/// Validate that parameters match a registered production BFV profile.
///
/// # Errors
/// Returns [`BfvError`] when the parameter set is malformed or not registered.
pub fn validate_registered_bfv_parameters(params: &BfvParameters) -> Result<(), BfvError> {
    params.validate()?;
    if *params != ram_lfe_bfv_parameters_v1() {
        return Err(BfvError::InvalidParameters(
            "BFV parameter set is not registered for production FHE evaluation".to_owned(),
        ));
    }
    Ok(())
}

/// Return the stable digest for a registered BFV parameter set.
///
/// # Errors
/// Returns [`BfvError`] when the parameter set is not registered.
pub fn registered_bfv_parameter_digest(params: &BfvParameters) -> Result<Hash, BfvError> {
    validate_registered_bfv_parameters(params)?;
    let bytes = norito::to_bytes(params)
        .map_err(|err| BfvError::InvalidParameters(format!("parameter encoding failed: {err}")))?;
    Ok(Hash::new(
        [BFV_PARAMETER_DIGEST_DOMAIN, bytes.as_slice()].concat(),
    ))
}

/// Public affine circuit over scalar ciphertext inputs.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct BfvAffineCircuit {
    /// Output rows; each row stores one plaintext weight per input ciphertext.
    pub weights: Vec<Vec<u64>>,
    /// Plaintext bias added to each output row.
    pub bias: Vec<u64>,
}

impl BfvAffineCircuit {
    /// Validate the circuit shape and plaintext coefficients.
    ///
    /// # Errors
    /// Returns [`BfvError`] when the circuit shape is invalid.
    pub fn validate(&self, params: &BfvParameters, input_count: usize) -> Result<(), BfvError> {
        if self.weights.is_empty() {
            return Err(BfvError::InvalidCircuit(
                "affine circuit must have at least one output row".to_owned(),
            ));
        }
        if self.weights.len() != self.bias.len() {
            return Err(BfvError::InvalidCircuit(
                "weights and bias must have the same outer length".to_owned(),
            ));
        }
        for (row_index, row) in self.weights.iter().enumerate() {
            if row.len() != input_count {
                return Err(BfvError::InvalidCircuit(format!(
                    "weights[{row_index}] expected {input_count} inputs, found {}",
                    row.len()
                )));
            }
            for &weight in row {
                if weight >= params.plaintext_modulus {
                    return Err(BfvError::InvalidCircuit(format!(
                        "weight {weight} exceeds plaintext modulus {}",
                        params.plaintext_modulus
                    )));
                }
            }
        }
        for &bias in &self.bias {
            if bias >= params.plaintext_modulus {
                return Err(BfvError::InvalidCircuit(format!(
                    "bias {bias} exceeds plaintext modulus {}",
                    params.plaintext_modulus
                )));
            }
        }
        Ok(())
    }
}

/// Errors raised by the deterministic BFV baseline.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum BfvError {
    /// The parameter set is invalid.
    #[error("invalid BFV parameters: {0}")]
    InvalidParameters(String),
    /// The ciphertext or key shape does not match the parameter set.
    #[error("shape mismatch: {0}")]
    ShapeMismatch(String),
    /// Plaintext coefficients exceed the plaintext modulus.
    #[error("plaintext coefficient {coefficient} exceeds plaintext modulus {plaintext_modulus}")]
    PlaintextOutOfRange {
        /// Offending plaintext coefficient.
        coefficient: u64,
        /// Active plaintext modulus.
        plaintext_modulus: u64,
    },
    /// The affine circuit is invalid.
    #[error("invalid affine circuit: {0}")]
    InvalidCircuit(String),
    /// The identifier input does not fit the configured BFV envelope.
    #[error("identifier input exceeds the maximum supported length of {max_input_bytes} bytes")]
    InputTooLong {
        /// Maximum supported identifier byte length.
        max_input_bytes: u16,
    },
    /// The identifier envelope is malformed after decryption.
    #[error("invalid BFV identifier envelope: {0}")]
    InvalidIdentifierEncoding(String),
}

/// Deterministic BFV key generation from a seed.
///
/// # Errors
/// Returns [`BfvError`] when parameters are invalid.
pub fn keygen_from_seed(
    params: &BfvParameters,
    seed: &[u8],
) -> Result<(BfvSecretKey, BfvPublicKey, BfvRelinearizationKey), BfvError> {
    params.validate()?;

    let mut rng = derive_rng(KEYGEN_DOMAIN, seed);
    let secret = sample_small_poly(params, &mut rng);
    let a = sample_uniform_poly(params, &mut rng);
    let e = sample_error_poly(params, &mut rng);
    let as_product = poly_mul_mod(params, &a, &secret);
    let b = poly_sub_mod(params, &poly_neg_mod(params, &as_product), &e);

    let secret_sq = poly_mul_mod(params, &secret, &secret);
    let digits = params.decomposition_digits();
    let base = params.decomposition_base();
    let mut scale = 1_u64;
    let mut relin_entries = Vec::with_capacity(digits);
    for _ in 0..digits {
        let relin_a = sample_uniform_poly(params, &mut rng);
        let relin_e = sample_error_poly(params, &mut rng);
        let scaled_secret_sq = poly_scalar_mul_mod(params, &secret_sq, scale);
        let relin_b = poly_add_mod(
            params,
            &poly_sub_mod(
                params,
                &poly_neg_mod(params, &poly_mul_mod(params, &relin_a, &secret)),
                &relin_e,
            ),
            &scaled_secret_sq,
        );
        relin_entries.push(BfvRelinearizationKeyEntry {
            b: relin_b,
            a: relin_a,
        });
        scale = mul_mod_u64(scale, base, params.ciphertext_modulus);
    }

    Ok((
        BfvSecretKey { s: secret },
        BfvPublicKey { b, a },
        BfvRelinearizationKey {
            entries: relin_entries,
        },
    ))
}

/// Encrypt a plaintext polynomial from a seed.
///
/// The plaintext is encoded coefficient-wise under the plaintext modulus. The
/// caller may pass fewer than `polynomial_degree` coefficients; the remainder
/// are treated as zero.
///
/// # Errors
/// Returns [`BfvError`] when parameters, plaintext, or key shapes are invalid.
pub fn encrypt_from_seed(
    params: &BfvParameters,
    public_key: &BfvPublicKey,
    plaintext: &[u64],
    seed: &[u8],
) -> Result<BfvCiphertext, BfvError> {
    params.validate()?;
    validate_public_key(params, public_key)?;
    validate_plaintext(params, plaintext)?;

    let encoded_plaintext = encode_plaintext(params, plaintext);
    let mut rng = derive_rng(ENCRYPT_DOMAIN, seed);
    let u = sample_small_poly(params, &mut rng);
    let e1 = sample_error_poly(params, &mut rng);
    let e2 = sample_error_poly(params, &mut rng);
    let c0 = poly_add_mod(
        params,
        &poly_add_mod(params, &poly_mul_mod(params, &public_key.b, &u), &e1),
        &encoded_plaintext,
    );
    let c1 = poly_add_mod(params, &poly_mul_mod(params, &public_key.a, &u), &e2);
    Ok(BfvCiphertext { c0, c1 })
}

/// Derive a deterministic public bootstrap refresh key from a BFV public key.
///
/// The resulting key contains an encryption of zero. Adding it to any
/// ciphertext under the same parameters preserves the plaintext while changing
/// the ciphertext bytes. This is the deterministic in-repo refresh primitive
/// used by Soracloud Bootstrap jobs; it keeps evaluators secret-key free.
/// TODO: Replace this encrypted-zero refresh with full BFV-RNS bootstrapping
/// once the RNS modulus-chain engine lands.
///
/// # Errors
/// Returns [`BfvError`] when parameter or public-key validation fails.
pub fn bootstrap_key_from_seed(
    params: &BfvParameters,
    public_key: &BfvPublicKey,
    key_id: impl Into<String>,
    seed: &[u8],
) -> Result<BfvBootstrapKey, BfvError> {
    let key_id = key_id.into();
    validate_bootstrap_key_id(&key_id)?;
    let zero_refresh = encrypt_from_seed(params, public_key, &[0], seed)?;
    let bootstrap_key = BfvBootstrapKey {
        key_id,
        zero_refresh,
    };
    validate_ciphertext(params, &bootstrap_key.zero_refresh)?;
    Ok(bootstrap_key)
}

/// Refresh a ciphertext with a public bootstrap key.
///
/// # Errors
/// Returns [`BfvError`] when the input or refresh key does not match the
/// parameter set.
pub fn bootstrap_ciphertext(
    params: &BfvParameters,
    bootstrap_key: &BfvBootstrapKey,
    ciphertext: &BfvCiphertext,
) -> Result<BfvCiphertext, BfvError> {
    params.validate()?;
    validate_bootstrap_key_id(&bootstrap_key.key_id)?;
    validate_ciphertext(params, &bootstrap_key.zero_refresh)?;
    add_ciphertexts(params, ciphertext, &bootstrap_key.zero_refresh)
}

/// Derive a deterministic public slot-rotation key from a BFV public key.
///
/// The resulting key contains an encryption of zero used to refresh every
/// ciphertext moved by an outer envelope slot rotation.
///
/// # Errors
/// Returns [`BfvError`] when the rotation step, parameter set, or public-key
/// shape is invalid.
pub fn rotation_key_from_seed(
    params: &BfvParameters,
    public_key: &BfvPublicKey,
    rotation_steps: u32,
    seed: &[u8],
) -> Result<BfvRotationKey, BfvError> {
    if rotation_steps == 0 {
        return Err(BfvError::InvalidParameters(
            "rotation key steps must be greater than zero".to_owned(),
        ));
    }
    let zero_refresh = encrypt_from_seed(params, public_key, &[0], seed)?;
    let rotation_key = BfvRotationKey {
        rotation_steps,
        zero_refresh,
    };
    validate_ciphertext(params, &rotation_key.zero_refresh)?;
    Ok(rotation_key)
}

/// Rotate an identifier ciphertext-slot envelope left and refresh each moved slot.
///
/// This helper implements Soracloud `RotateLeft`: it rotates the outer vector of
/// BFV scalar ciphertext slots by the key's declared step count, then adds the
/// key's encrypted-zero refresh mask to every output slot. The plaintext slot
/// order changes, ciphertext bytes change, and the evaluator never needs a
/// BFV secret key.
///
/// # Errors
/// Returns [`BfvError`] when the key or any ciphertext slot does not match the
/// parameter set.
pub fn rotate_ciphertext_slots_left(
    params: &BfvParameters,
    rotation_key: &BfvRotationKey,
    slots: &[BfvCiphertext],
) -> Result<Vec<BfvCiphertext>, BfvError> {
    params.validate()?;
    if rotation_key.rotation_steps == 0 {
        return Err(BfvError::InvalidParameters(
            "rotation key steps must be greater than zero".to_owned(),
        ));
    }
    validate_ciphertext(params, &rotation_key.zero_refresh)?;
    for slot in slots {
        validate_ciphertext(params, slot)?;
    }
    if slots.is_empty() {
        return Ok(Vec::new());
    }

    let mut rotated = slots.to_vec();
    let slot_count = rotated.len();
    rotated.rotate_left(
        usize::try_from(rotation_key.rotation_steps).unwrap_or(usize::MAX) % slot_count,
    );
    rotated
        .iter()
        .map(|slot| add_ciphertexts(params, slot, &rotation_key.zero_refresh))
        .collect()
}

/// Decrypt a ciphertext back into plaintext coefficients.
///
/// # Errors
/// Returns [`BfvError`] when parameters, ciphertext, or key shapes are invalid.
pub fn decrypt(
    params: &BfvParameters,
    secret_key: &BfvSecretKey,
    ciphertext: &BfvCiphertext,
) -> Result<Vec<u64>, BfvError> {
    params.validate()?;
    validate_secret_key(params, secret_key)?;
    validate_ciphertext(params, ciphertext)?;

    let scaled = poly_add_mod(
        params,
        &ciphertext.c0,
        &poly_mul_mod(params, &ciphertext.c1, &secret_key.s),
    );
    Ok(decode_plaintext(params, &scaled))
}

/// Homomorphically add two ciphertexts.
///
/// # Errors
/// Returns [`BfvError`] when ciphertext shapes do not match the parameter set.
pub fn add_ciphertexts(
    params: &BfvParameters,
    lhs: &BfvCiphertext,
    rhs: &BfvCiphertext,
) -> Result<BfvCiphertext, BfvError> {
    params.validate()?;
    validate_ciphertext(params, lhs)?;
    validate_ciphertext(params, rhs)?;
    Ok(BfvCiphertext {
        c0: poly_add_mod(params, &lhs.c0, &rhs.c0),
        c1: poly_add_mod(params, &lhs.c1, &rhs.c1),
    })
}

/// Homomorphically subtract one ciphertext from another.
///
/// # Errors
/// Returns [`BfvError`] when ciphertext shapes do not match the parameter set.
pub fn subtract_ciphertexts(
    params: &BfvParameters,
    lhs: &BfvCiphertext,
    rhs: &BfvCiphertext,
) -> Result<BfvCiphertext, BfvError> {
    params.validate()?;
    validate_ciphertext(params, lhs)?;
    validate_ciphertext(params, rhs)?;
    Ok(BfvCiphertext {
        c0: poly_sub_mod(params, &lhs.c0, &rhs.c0),
        c1: poly_sub_mod(params, &lhs.c1, &rhs.c1),
    })
}

/// Add a plaintext scalar to the coefficient-0 slot of a ciphertext.
///
/// # Errors
/// Returns [`BfvError`] when the plaintext or ciphertext shape is invalid.
pub fn add_plain_scalar(
    params: &BfvParameters,
    ciphertext: &BfvCiphertext,
    scalar: u64,
) -> Result<BfvCiphertext, BfvError> {
    params.validate()?;
    validate_ciphertext(params, ciphertext)?;
    if scalar >= params.plaintext_modulus {
        return Err(BfvError::PlaintextOutOfRange {
            coefficient: scalar,
            plaintext_modulus: params.plaintext_modulus,
        });
    }
    let mut encoded = zero_poly(params);
    encoded[0] = scalar % params.plaintext_modulus;
    Ok(BfvCiphertext {
        c0: poly_add_mod(params, &ciphertext.c0, &encoded),
        c1: ciphertext.c1.clone(),
    })
}

/// Multiply a ciphertext by a plaintext scalar modulo the plaintext modulus.
///
/// # Errors
/// Returns [`BfvError`] when the plaintext or ciphertext shape is invalid.
pub fn multiply_plain_scalar(
    params: &BfvParameters,
    ciphertext: &BfvCiphertext,
    scalar: u64,
) -> Result<BfvCiphertext, BfvError> {
    params.validate()?;
    validate_ciphertext(params, ciphertext)?;
    if scalar >= params.plaintext_modulus {
        return Err(BfvError::PlaintextOutOfRange {
            coefficient: scalar,
            plaintext_modulus: params.plaintext_modulus,
        });
    }
    Ok(BfvCiphertext {
        c0: poly_scalar_mul_mod(params, &ciphertext.c0, scalar),
        c1: poly_scalar_mul_mod(params, &ciphertext.c1, scalar),
    })
}

/// Multiply two ciphertexts and relinearize the result back to two components.
///
/// # Errors
/// Returns [`BfvError`] when parameters or operand shapes are invalid.
pub fn multiply_ciphertexts(
    params: &BfvParameters,
    relinearization_key: &BfvRelinearizationKey,
    lhs: &BfvCiphertext,
    rhs: &BfvCiphertext,
) -> Result<BfvCiphertext, BfvError> {
    params.validate()?;
    validate_ciphertext(params, lhs)?;
    validate_ciphertext(params, rhs)?;
    validate_relinearization_key(params, relinearization_key)?;

    let raw_c0 = poly_mul_mod(params, &lhs.c0, &rhs.c0);
    let raw_c1 = poly_add_mod(
        params,
        &poly_mul_mod(params, &lhs.c0, &rhs.c1),
        &poly_mul_mod(params, &lhs.c1, &rhs.c0),
    );
    let raw_c2 = poly_mul_mod(params, &lhs.c1, &rhs.c1);
    Ok(relinearize(
        params,
        relinearization_key,
        &raw_c0,
        &raw_c1,
        &raw_c2,
    ))
}

/// Evaluate a public affine circuit over scalar ciphertext inputs.
///
/// Each input ciphertext is expected to encode its scalar in coefficient 0. The
/// returned ciphertexts follow the same convention.
///
/// # Errors
/// Returns [`BfvError`] when parameters, input ciphertexts, or circuit shapes are invalid.
pub fn evaluate_affine_circuit(
    params: &BfvParameters,
    circuit: &BfvAffineCircuit,
    inputs: &[BfvCiphertext],
) -> Result<Vec<BfvCiphertext>, BfvError> {
    params.validate()?;
    for ciphertext in inputs {
        validate_ciphertext(params, ciphertext)?;
    }
    circuit.validate(params, inputs.len())?;

    let mut outputs = Vec::with_capacity(circuit.weights.len());
    for (row, &bias) in circuit.weights.iter().zip(&circuit.bias) {
        let mut accumulator = zero_ciphertext(params);
        for (ciphertext, &weight) in inputs.iter().zip(row) {
            let weighted = multiply_plain_scalar(params, ciphertext, weight)?;
            accumulator = add_ciphertexts(params, &accumulator, &weighted)?;
        }
        outputs.push(add_plain_scalar(params, &accumulator, bias)?);
    }
    Ok(outputs)
}

/// Derive deterministic BFV key material for encrypted identifier input.
///
/// The derived public parameters are suitable for publication in policy
/// metadata, while the secret key remains private to the resolver runtime.
///
/// # Errors
/// Returns [`BfvError`] when parameters or envelope capacity are invalid.
pub fn derive_identifier_key_material_from_seed(
    params: &BfvParameters,
    max_input_bytes: u16,
    seed: &[u8],
    associated_data: &[u8],
) -> Result<
    (
        BfvIdentifierPublicParameters,
        BfvSecretKey,
        BfvRelinearizationKey,
    ),
    BfvError,
> {
    let derived_seed = Hash::new([IDENTIFIER_KEYGEN_DOMAIN, associated_data, seed].concat());
    let derived_seed: [u8; Hash::LENGTH] = derived_seed.into();
    let (secret_key, public_key, relinearization_key) = keygen_from_seed(params, &derived_seed)?;
    let public_parameters = BfvIdentifierPublicParameters {
        parameters: *params,
        public_key,
        max_input_bytes,
    };
    public_parameters.validate()?;
    Ok((public_parameters, secret_key, relinearization_key))
}

/// Encrypt raw identifier bytes into a BFV ciphertext envelope.
///
/// The envelope stores the byte length in slot 0 followed by one byte per slot.
///
/// # Errors
/// Returns [`BfvError`] when the input does not fit the configured envelope.
pub fn encrypt_identifier_from_seed(
    public_parameters: &BfvIdentifierPublicParameters,
    input: &[u8],
    seed: &[u8],
) -> Result<BfvIdentifierCiphertext, BfvError> {
    public_parameters.validate()?;
    let scalars = encode_identifier_slots(public_parameters, input)?;
    let slots = scalars
        .into_iter()
        .enumerate()
        .map(|(index, scalar)| {
            let slot_seed = derive_identifier_slot_seed(seed, index)?;
            encrypt_from_seed(
                &public_parameters.parameters,
                &public_parameters.public_key,
                &[scalar],
                &slot_seed,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(BfvIdentifierCiphertext { slots })
}

/// Decrypt a BFV ciphertext envelope back into raw identifier bytes.
///
/// # Errors
/// Returns [`BfvError`] when the ciphertext or decrypted envelope is invalid.
pub fn decrypt_identifier(
    public_parameters: &BfvIdentifierPublicParameters,
    secret_key: &BfvSecretKey,
    ciphertext: &BfvIdentifierCiphertext,
) -> Result<Vec<u8>, BfvError> {
    public_parameters.validate()?;
    let expected_slots = usize::from(public_parameters.max_input_bytes).saturating_add(1);
    if ciphertext.slots.len() != expected_slots {
        return Err(BfvError::ShapeMismatch(format!(
            "identifier ciphertext expected {expected_slots} slots, found {}",
            ciphertext.slots.len()
        )));
    }
    let scalars = ciphertext
        .slots
        .iter()
        .map(|slot| decrypt_identifier_slot(public_parameters, secret_key, slot))
        .collect::<Result<Vec<_>, _>>()?;
    decode_identifier_slots(public_parameters, &scalars)
}

/// Human-readable summary of one ciphertext slot decoded as a scalar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BfvScalar(pub u64);

impl fmt::Display for BfvScalar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn validate_plaintext(params: &BfvParameters, plaintext: &[u64]) -> Result<(), BfvError> {
    if plaintext.len() > params.degree() {
        return Err(BfvError::ShapeMismatch(format!(
            "plaintext length {} exceeds polynomial_degree {}",
            plaintext.len(),
            params.polynomial_degree
        )));
    }
    for &coefficient in plaintext {
        if coefficient >= params.plaintext_modulus {
            return Err(BfvError::PlaintextOutOfRange {
                coefficient,
                plaintext_modulus: params.plaintext_modulus,
            });
        }
    }
    Ok(())
}

fn validate_secret_key(params: &BfvParameters, secret_key: &BfvSecretKey) -> Result<(), BfvError> {
    validate_poly(params, &secret_key.s, "secret key")
}

fn validate_public_key(params: &BfvParameters, public_key: &BfvPublicKey) -> Result<(), BfvError> {
    validate_poly(params, &public_key.b, "public key b")?;
    validate_poly(params, &public_key.a, "public key a")
}

fn validate_relinearization_key(
    params: &BfvParameters,
    relinearization_key: &BfvRelinearizationKey,
) -> Result<(), BfvError> {
    if relinearization_key.entries.len() != params.decomposition_digits() {
        return Err(BfvError::ShapeMismatch(format!(
            "relinearization key expected {} entries, found {}",
            params.decomposition_digits(),
            relinearization_key.entries.len()
        )));
    }
    for (index, entry) in relinearization_key.entries.iter().enumerate() {
        validate_poly(params, &entry.b, &format!("relinearization key b[{index}]"))?;
        validate_poly(params, &entry.a, &format!("relinearization key a[{index}]"))?;
    }
    Ok(())
}

fn validate_ciphertext(params: &BfvParameters, ciphertext: &BfvCiphertext) -> Result<(), BfvError> {
    validate_poly(params, &ciphertext.c0, "ciphertext c0")?;
    validate_poly(params, &ciphertext.c1, "ciphertext c1")
}

fn validate_poly(params: &BfvParameters, poly: &[u64], label: &str) -> Result<(), BfvError> {
    if poly.len() != params.degree() {
        return Err(BfvError::ShapeMismatch(format!(
            "{label} length {} does not match polynomial_degree {}",
            poly.len(),
            params.polynomial_degree
        )));
    }
    if poly
        .iter()
        .any(|&coefficient| coefficient >= params.ciphertext_modulus)
    {
        return Err(BfvError::ShapeMismatch(format!(
            "{label} contains a coefficient outside ciphertext modulus {}",
            params.ciphertext_modulus
        )));
    }
    Ok(())
}

fn validate_bootstrap_key_id(key_id: &str) -> Result<(), BfvError> {
    if key_id.is_empty() {
        return Err(BfvError::InvalidParameters(
            "bootstrap key id must not be empty".to_owned(),
        ));
    }
    if key_id.len() > BFV_BOOTSTRAP_KEY_ID_MAX_BYTES {
        return Err(BfvError::InvalidParameters(format!(
            "bootstrap key id exceeds the maximum supported length {BFV_BOOTSTRAP_KEY_ID_MAX_BYTES}"
        )));
    }
    if key_id.trim() != key_id {
        return Err(BfvError::InvalidParameters(
            "bootstrap key id must be canonical without surrounding whitespace".to_owned(),
        ));
    }
    if !key_id.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(BfvError::InvalidParameters(
            "bootstrap key id must contain only printable ASCII bytes".to_owned(),
        ));
    }
    Ok(())
}

fn derive_rng(domain: &[u8], seed: &[u8]) -> ChaCha20Rng {
    let mut transcript = Vec::with_capacity(domain.len() + seed.len());
    transcript.extend_from_slice(domain);
    transcript.extend_from_slice(seed);
    let material: [u8; Hash::LENGTH] = Hash::new(transcript).into();
    ChaCha20Rng::from_seed(material)
}

fn encode_identifier_slots(
    public_parameters: &BfvIdentifierPublicParameters,
    input: &[u8],
) -> Result<Vec<u64>, BfvError> {
    if input.len() > usize::from(public_parameters.max_input_bytes) {
        return Err(BfvError::InputTooLong {
            max_input_bytes: public_parameters.max_input_bytes,
        });
    }
    let mut slots = vec![0_u64; usize::from(public_parameters.max_input_bytes).saturating_add(1)];
    slots[0] = u64::try_from(input.len()).map_err(|_| {
        BfvError::InvalidIdentifierEncoding(
            "identifier byte length does not fit into u64".to_owned(),
        )
    })?;
    for (index, byte) in input.iter().enumerate() {
        slots[index + 1] = u64::from(*byte);
    }
    Ok(slots)
}

fn decode_identifier_slots(
    public_parameters: &BfvIdentifierPublicParameters,
    slots: &[u64],
) -> Result<Vec<u8>, BfvError> {
    let expected_slots = usize::from(public_parameters.max_input_bytes).saturating_add(1);
    if slots.len() != expected_slots {
        return Err(BfvError::InvalidIdentifierEncoding(format!(
            "identifier slot count {} does not match expected {expected_slots}",
            slots.len()
        )));
    }
    let declared_len = usize::try_from(slots[0]).map_err(|_| {
        BfvError::InvalidIdentifierEncoding("identifier length does not fit into usize".to_owned())
    })?;
    if declared_len > usize::from(public_parameters.max_input_bytes) {
        return Err(BfvError::InvalidIdentifierEncoding(format!(
            "identifier length {declared_len} exceeds max_input_bytes {}",
            public_parameters.max_input_bytes
        )));
    }
    if slots[declared_len + 1..].iter().any(|&slot| slot != 0) {
        return Err(BfvError::InvalidIdentifierEncoding(
            "identifier ciphertext contains non-zero trailing slots".to_owned(),
        ));
    }
    slots[1..=declared_len]
        .iter()
        .map(|&slot| {
            u8::try_from(slot).map_err(|_| {
                BfvError::InvalidIdentifierEncoding(format!(
                    "identifier byte slot {slot} does not fit into u8"
                ))
            })
        })
        .collect()
}

fn decrypt_identifier_slot(
    public_parameters: &BfvIdentifierPublicParameters,
    secret_key: &BfvSecretKey,
    slot: &BfvCiphertext,
) -> Result<u64, BfvError> {
    let plaintext = decrypt(&public_parameters.parameters, secret_key, slot)?;
    if plaintext.first().copied().unwrap_or(0) >= public_parameters.parameters.plaintext_modulus {
        return Err(BfvError::InvalidIdentifierEncoding(
            "identifier slot does not fit into the plaintext modulus".to_owned(),
        ));
    }
    if plaintext
        .iter()
        .skip(1)
        .any(|&coefficient| coefficient != 0)
    {
        return Err(BfvError::InvalidIdentifierEncoding(
            "identifier slot contains non-zero trailing coefficients".to_owned(),
        ));
    }
    Ok(plaintext[0])
}

fn derive_identifier_slot_seed(seed: &[u8], index: usize) -> Result<[u8; Hash::LENGTH], BfvError> {
    let index = u64::try_from(index).map_err(|_| {
        BfvError::InvalidIdentifierEncoding(
            "identifier slot index does not fit into u64".to_owned(),
        )
    })?;
    Ok(Hash::new([IDENTIFIER_SLOT_ENCRYPT_DOMAIN, seed, &index.to_le_bytes()].concat()).into())
}

fn zero_poly(params: &BfvParameters) -> Polynomial {
    vec![0; params.degree()]
}

fn zero_ciphertext(params: &BfvParameters) -> BfvCiphertext {
    BfvCiphertext {
        c0: zero_poly(params),
        c1: zero_poly(params),
    }
}

fn sample_small_poly(params: &BfvParameters, rng: &mut ChaCha20Rng) -> Polynomial {
    (0..params.degree())
        .map(|_| match rng.random_range(0..=2_u8) {
            0 => 0,
            1 => 1,
            _ => params.ciphertext_modulus - 1,
        })
        .collect()
}

fn sample_error_poly(params: &BfvParameters, rng: &mut ChaCha20Rng) -> Polynomial {
    let _ = rng;
    // TODO: Replace this exact zero-error release profile with bounded RLWE
    // noise once the full BFV-RNS modulus-chain and bootstrapping engine lands.
    zero_poly(params)
}

fn sample_uniform_poly(params: &BfvParameters, rng: &mut ChaCha20Rng) -> Polynomial {
    (0..params.degree())
        .map(|_| rng.random_range(0..params.ciphertext_modulus))
        .collect()
}

fn encode_plaintext(params: &BfvParameters, plaintext: &[u64]) -> Polynomial {
    let mut encoded = zero_poly(params);
    for (slot, &coefficient) in plaintext.iter().enumerate() {
        encoded[slot] = coefficient % params.plaintext_modulus;
    }
    encoded
}

fn decode_plaintext(params: &BfvParameters, scaled: &[u64]) -> Vec<u64> {
    scaled
        .iter()
        .map(|&coefficient| {
            let centered = center_lift(coefficient, params.ciphertext_modulus);
            mod_t(centered, params.plaintext_modulus)
        })
        .collect()
}

fn relinearize(
    params: &BfvParameters,
    relinearization_key: &BfvRelinearizationKey,
    c0: &[u64],
    c1: &[u64],
    c2: &[u64],
) -> BfvCiphertext {
    let digits = decompose_poly(params, c2);
    let mut out0 = c0.to_vec();
    let mut out1 = c1.to_vec();
    for (digit_poly, entry) in digits.iter().zip(&relinearization_key.entries) {
        out0 = poly_add_mod(params, &out0, &poly_mul_mod(params, digit_poly, &entry.b));
        out1 = poly_add_mod(params, &out1, &poly_mul_mod(params, digit_poly, &entry.a));
    }
    BfvCiphertext { c0: out0, c1: out1 }
}

fn decompose_poly(params: &BfvParameters, poly: &[u64]) -> Vec<Polynomial> {
    let digits = params.decomposition_digits();
    let base = params.decomposition_base();
    let mut output = vec![zero_poly(params); digits];
    for (coeff_index, &coefficient) in poly.iter().enumerate() {
        let mut value = coefficient;
        for digit_poly in &mut output {
            digit_poly[coeff_index] = value % base;
            value /= base;
        }
    }
    output
}

fn poly_add_mod(params: &BfvParameters, lhs: &[u64], rhs: &[u64]) -> Polynomial {
    lhs.iter()
        .zip(rhs)
        .map(|(&left, &right)| add_mod_u64(left, right, params.ciphertext_modulus))
        .collect()
}

fn poly_sub_mod(params: &BfvParameters, lhs: &[u64], rhs: &[u64]) -> Polynomial {
    lhs.iter()
        .zip(rhs)
        .map(|(&left, &right)| sub_mod_u64(left, right, params.ciphertext_modulus))
        .collect()
}

fn poly_neg_mod(params: &BfvParameters, poly: &[u64]) -> Polynomial {
    poly.iter()
        .map(|&coefficient| {
            if coefficient == 0 {
                0
            } else {
                params.ciphertext_modulus - coefficient
            }
        })
        .collect()
}

fn poly_scalar_mul_mod(params: &BfvParameters, poly: &[u64], scalar: u64) -> Polynomial {
    poly.iter()
        .map(|&coefficient| mul_mod_u64(coefficient, scalar, params.ciphertext_modulus))
        .collect()
}

fn poly_mul_mod(params: &BfvParameters, lhs: &[u64], rhs: &[u64]) -> Polynomial {
    poly_mul_raw(params, lhs, rhs)
        .into_iter()
        .map(|coefficient| mod_q(coefficient, params.ciphertext_modulus))
        .collect()
}

fn poly_mul_raw(params: &BfvParameters, lhs: &[u64], rhs: &[u64]) -> Vec<i128> {
    #[cfg(feature = "bfv-accel")]
    if matches!(params.convolution_backend(), BfvConvolutionBackend::CrtNtt) {
        return poly_mul_raw_crt_ntt(params, lhs, rhs);
    }
    poly_mul_raw_scalar(params, lhs, rhs)
}

fn poly_mul_raw_scalar(params: &BfvParameters, lhs: &[u64], rhs: &[u64]) -> Vec<i128> {
    let lhs = lhs
        .iter()
        .map(|&value| i128::from(value))
        .collect::<Vec<_>>();
    let rhs = rhs
        .iter()
        .map(|&value| i128::from(value))
        .collect::<Vec<_>>();
    poly_mul_raw_scalar_i128(params, &lhs, &rhs)
}

fn poly_mul_raw_scalar_i128(params: &BfvParameters, lhs: &[i128], rhs: &[i128]) -> Vec<i128> {
    let n = params.degree();
    let mut acc = vec![0_i128; n];
    for (i, &left) in lhs.iter().enumerate() {
        for (j, &right) in rhs.iter().enumerate() {
            let index = i + j;
            let term = left * right;
            if index < n {
                acc[index] += term;
            } else {
                acc[index - n] -= term;
            }
        }
    }
    acc
}

#[cfg(feature = "bfv-accel")]
fn poly_mul_raw_crt_ntt(params: &BfvParameters, lhs: &[u64], rhs: &[u64]) -> Vec<i128> {
    try_poly_mul_raw_crt_ntt(params, lhs, rhs)
        .unwrap_or_else(|| poly_mul_raw_scalar(params, lhs, rhs))
}

#[cfg(feature = "bfv-accel")]
fn try_poly_mul_raw_crt_ntt(params: &BfvParameters, lhs: &[u64], rhs: &[u64]) -> Option<Vec<i128>> {
    let n = params.degree();
    if n == 0 || lhs.len() != n || rhs.len() != n {
        return None;
    }
    let linear = convolve_linear_crt_ntt(lhs, rhs)?;
    let mut folded = vec![0_i128; n];
    for (index, slot) in folded.iter_mut().enumerate() {
        let low = i128::try_from(*linear.get(index)?).ok()?;
        let high = i128::try_from(*linear.get(index + n)?).ok()?;
        *slot = low - high;
    }
    Some(folded)
}

#[cfg(feature = "bfv-accel")]
fn convolve_linear_crt_ntt(lhs: &[u64], rhs: &[u64]) -> Option<Vec<u128>> {
    let len = lhs.len().checked_mul(2)?;
    if len == 0 || !len.is_power_of_two() {
        return None;
    }
    let required_log = len.ilog2();
    let mut residues = Vec::with_capacity(CRT_NTT_PRIMES.len());
    for prime in CRT_NTT_PRIMES {
        if required_log > prime.max_power_of_two {
            return None;
        }
        residues.push(convolve_linear_mod_prime(lhs, rhs, len, prime)?);
    }
    (0..len)
        .map(|index| {
            let coeffs = [
                residues[0][index],
                residues[1][index],
                residues[2][index],
                residues[3][index],
            ];
            garner_reconstruct_u128(&coeffs, &CRT_NTT_PRIMES)
        })
        .collect::<Option<Vec<_>>>()
}

#[cfg(feature = "bfv-accel")]
fn convolve_linear_mod_prime(
    lhs: &[u64],
    rhs: &[u64],
    len: usize,
    prime: NttPrime,
) -> Option<Vec<u64>> {
    if len == 0 || !len.is_power_of_two() || len.ilog2() > prime.max_power_of_two {
        return None;
    }
    let modulus = prime.modulus;
    let mut lhs_ntt = vec![0_u64; len];
    let mut rhs_ntt = vec![0_u64; len];
    for (slot, &coefficient) in lhs_ntt.iter_mut().zip(lhs) {
        *slot = coefficient % modulus;
    }
    for (slot, &coefficient) in rhs_ntt.iter_mut().zip(rhs) {
        *slot = coefficient % modulus;
    }
    ntt_in_place(&mut lhs_ntt, prime, false)?;
    ntt_in_place(&mut rhs_ntt, prime, false)?;
    for (left, right) in lhs_ntt.iter_mut().zip(&rhs_ntt) {
        *left = mul_mod_prime(*left, *right, modulus);
    }
    ntt_in_place(&mut lhs_ntt, prime, true)?;
    Some(lhs_ntt)
}

#[cfg(feature = "bfv-accel")]
fn ntt_in_place(values: &mut [u64], prime: NttPrime, invert: bool) -> Option<()> {
    let len = values.len();
    let modulus = prime.modulus;
    let root = root_for_length(prime, len)?;
    bit_reverse_permute(values);
    let root = if invert {
        mod_inv_prime(root, modulus)
    } else {
        root
    };

    let mut stage_len = 2_usize;
    while stage_len <= len {
        let step = mod_pow_prime(root, u64::try_from(len / stage_len).ok()?, modulus);
        for chunk in values.chunks_exact_mut(stage_len) {
            let (lo, hi) = chunk.split_at_mut(stage_len / 2);
            let mut twiddle = 1_u64;
            for (left, right) in lo.iter_mut().zip(hi.iter_mut()) {
                let product = mul_mod_prime(*right, twiddle, modulus);
                let left_value = *left;
                *left = add_mod_prime(left_value, product, modulus);
                *right = sub_mod_prime(left_value, product, modulus);
                twiddle = mul_mod_prime(twiddle, step, modulus);
            }
        }
        stage_len = match stage_len.checked_mul(2) {
            Some(next) => next,
            None => break,
        };
    }

    if invert {
        let inv_len = mod_inv_prime(u64::try_from(len).ok()?, modulus);
        for value in values {
            *value = mul_mod_prime(*value, inv_len, modulus);
        }
    }
    Some(())
}

#[cfg(feature = "bfv-accel")]
fn root_for_length(prime: NttPrime, len: usize) -> Option<u64> {
    if len == 0 || prime.modulus <= 1 {
        return None;
    }
    let log_len = len.ilog2();
    if !len.is_power_of_two() || log_len > prime.max_power_of_two {
        return None;
    }
    let len = u64::try_from(len).ok()?;
    Some(mod_pow_prime(
        prime.primitive_root,
        (prime.modulus - 1) / len,
        prime.modulus,
    ))
}

#[cfg(feature = "bfv-accel")]
fn bit_reverse_permute(values: &mut [u64]) {
    if values.is_empty() {
        return;
    }
    let bits = values.len().ilog2();
    for index in 0..values.len() {
        let reversed = index.reverse_bits() >> (usize::BITS - bits);
        if reversed > index {
            values.swap(index, reversed);
        }
    }
}

#[cfg(feature = "bfv-accel")]
fn garner_reconstruct_u128(residues: &[u64], primes: &[NttPrime]) -> Option<u128> {
    if residues.len() != primes.len() {
        return None;
    }
    let mut mixed = vec![0_u64; residues.len()];
    for (index, (&residue, prime)) in residues.iter().zip(primes).enumerate() {
        let mut coefficient = residue;
        for (prior, prior_prime) in mixed[..index].iter().zip(primes.iter()) {
            coefficient = mul_mod_prime(
                sub_mod_prime(coefficient, *prior, prime.modulus),
                mod_inv_prime(prior_prime.modulus % prime.modulus, prime.modulus),
                prime.modulus,
            );
        }
        mixed[index] = coefficient;
    }
    let mut value = 0_u128;
    let mut weight = 1_u128;
    for (index, coefficient) in mixed.iter().enumerate() {
        let term = u128::from(*coefficient).checked_mul(weight)?;
        value = value.checked_add(term)?;
        if index + 1 != mixed.len() {
            weight = weight.checked_mul(u128::from(primes[index].modulus))?;
        }
    }
    Some(value)
}

#[cfg(feature = "bfv-accel")]
fn add_mod_prime(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    if modulus == 0 {
        return 0;
    }
    low_u64_from_u128((u128::from(lhs) + u128::from(rhs)) % u128::from(modulus))
}

#[cfg(feature = "bfv-accel")]
fn sub_mod_prime(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    if modulus == 0 {
        return 0;
    }
    let lhs = lhs % modulus;
    let rhs = rhs % modulus;
    if lhs >= rhs {
        lhs - rhs
    } else {
        modulus - (rhs - lhs)
    }
}

#[cfg(feature = "bfv-accel")]
fn mul_mod_prime(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    if modulus == 0 {
        return 0;
    }
    low_u64_from_u128((u128::from(lhs) * u128::from(rhs)) % u128::from(modulus))
}

#[cfg(feature = "bfv-accel")]
fn mod_pow_prime(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    if modulus == 0 {
        return 0;
    }
    let mut result = 1_u64;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = mul_mod_prime(result, base, modulus);
        }
        base = mul_mod_prime(base, base, modulus);
        exponent >>= 1;
    }
    result
}

#[cfg(feature = "bfv-accel")]
fn mod_inv_prime(value: u64, modulus: u64) -> u64 {
    if modulus <= 1 {
        return 0;
    }
    mod_pow_prime(value, modulus - 2, modulus)
}

fn low_u64_from_u128(value: u128) -> u64 {
    let [b0, b1, b2, b3, b4, b5, b6, b7, _, _, _, _, _, _, _, _] = value.to_le_bytes();
    u64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, b7])
}

fn low_u64_from_i128(value: i128) -> u64 {
    let [b0, b1, b2, b3, b4, b5, b6, b7, _, _, _, _, _, _, _, _] = value.to_le_bytes();
    u64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, b7])
}

fn add_mod_u64(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    low_u64_from_u128((u128::from(lhs) + u128::from(rhs)) % u128::from(modulus))
}

fn sub_mod_u64(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    mod_q(i128::from(lhs) - i128::from(rhs), modulus)
}

fn mul_mod_u64(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    low_u64_from_u128((u128::from(lhs) * u128::from(rhs)) % u128::from(modulus))
}

fn mod_q(value: i128, modulus: u64) -> u64 {
    let modulus = i128::from(modulus);
    let reduced = value.rem_euclid(modulus);
    low_u64_from_i128(reduced)
}

fn mod_t(value: i128, modulus: u64) -> u64 {
    mod_q(value, modulus)
}

fn center_lift(coefficient: u64, modulus: u64) -> i128 {
    let coefficient = i128::from(coefficient);
    let modulus = i128::from(modulus);
    if coefficient > modulus / 2 {
        coefficient - modulus
    } else {
        coefficient
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> BfvParameters {
        BfvParameters {
            polynomial_degree: 8,
            ciphertext_modulus: 16_777_216,
            plaintext_modulus: 256,
            decomposition_base_log: 12,
        }
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let params = params();
        let (secret_key, public_key, _) =
            keygen_from_seed(&params, b"bfv-roundtrip-keygen").expect("keygen");
        let ciphertext = encrypt_from_seed(
            &params,
            &public_key,
            &[13, 42, 99, 7],
            b"bfv-roundtrip-encrypt",
        )
        .expect("encrypt");
        let plaintext = decrypt(&params, &secret_key, &ciphertext).expect("decrypt");
        assert_eq!(&plaintext[..4], &[13, 42, 99, 7]);
    }

    #[test]
    fn homomorphic_addition_matches_plaintext() {
        let params = params();
        let (secret_key, public_key, _) =
            keygen_from_seed(&params, b"bfv-add-keygen").expect("keygen");
        let lhs =
            encrypt_from_seed(&params, &public_key, &[17], b"bfv-add-left").expect("encrypt lhs");
        let rhs =
            encrypt_from_seed(&params, &public_key, &[29], b"bfv-add-right").expect("encrypt rhs");
        let sum = add_ciphertexts(&params, &lhs, &rhs).expect("add");
        let plaintext = decrypt(&params, &secret_key, &sum).expect("decrypt");
        assert_eq!(plaintext[0], 46);
    }

    #[test]
    fn bootstrap_refresh_preserves_plaintext_and_changes_ciphertext() {
        let params = params();
        let (secret_key, public_key, _) =
            keygen_from_seed(&params, b"bfv-bootstrap-keygen").expect("keygen");
        let ciphertext = encrypt_from_seed(
            &params,
            &public_key,
            &[77],
            b"bfv-bootstrap-input-ciphertext",
        )
        .expect("encrypt");
        let bootstrap_key = bootstrap_key_from_seed(
            &params,
            &public_key,
            "bootstrap-refresh-key",
            b"bfv-bootstrap-zero-refresh",
        )
        .expect("bootstrap key");
        let refreshed =
            bootstrap_ciphertext(&params, &bootstrap_key, &ciphertext).expect("bootstrap refresh");

        assert_ne!(refreshed, ciphertext);
        let plaintext = decrypt(&params, &secret_key, &refreshed).expect("decrypt");
        assert_eq!(plaintext[0], 77);
    }

    #[test]
    fn rotation_key_rotates_and_refreshes_ciphertext_slots() {
        let params = params();
        let (secret_key, public_key, _) =
            keygen_from_seed(&params, b"bfv-rotation-keygen").expect("keygen");
        let slots = [10_u64, 20, 30]
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                encrypt_from_seed(
                    &params,
                    &public_key,
                    &[value],
                    format!("bfv-rotation-slot-{index}").as_bytes(),
                )
                .expect("encrypt slot")
            })
            .collect::<Vec<_>>();
        let rotation_key =
            rotation_key_from_seed(&params, &public_key, 1, b"bfv-rotation-zero-refresh")
                .expect("rotation key");

        let rotated =
            rotate_ciphertext_slots_left(&params, &rotation_key, &slots).expect("rotate slots");

        assert_eq!(rotated.len(), slots.len());
        assert_ne!(
            rotated,
            vec![slots[1].clone(), slots[2].clone(), slots[0].clone()]
        );
        let plaintexts = rotated
            .iter()
            .map(|slot| decrypt(&params, &secret_key, slot).expect("decrypt")[0])
            .collect::<Vec<_>>();
        assert_eq!(plaintexts, vec![20, 30, 10]);
    }

    #[test]
    fn rotation_key_rejects_zero_steps() {
        let params = params();
        let (_, public_key, _) =
            keygen_from_seed(&params, b"bfv-rotation-zero-keygen").expect("keygen");
        let err = rotation_key_from_seed(&params, &public_key, 0, b"bfv-rotation-zero")
            .expect_err("zero-step rotation keys must fail");
        assert_eq!(
            err,
            BfvError::InvalidParameters("rotation key steps must be greater than zero".to_owned())
        );
    }

    #[test]
    fn evaluation_key_bundle_rejects_adversarial_key_metadata() {
        let params = params();
        let (_, public_key, relinearization_key) =
            keygen_from_seed(&params, b"bfv-eval-key-adversarial-keygen").expect("keygen");
        let rotation_key =
            rotation_key_from_seed(&params, &public_key, 1, b"bfv-eval-key-rotation")
                .expect("rotation key");
        let zero_refresh =
            encrypt_from_seed(&params, &public_key, &[0], b"bfv-eval-key-zero-refresh")
                .expect("encrypt zero");

        let duplicate_rotation = BfvEvaluationKeyBundle {
            relinearization_key: relinearization_key.clone(),
            rotation_keys: vec![rotation_key.clone(), rotation_key.clone()],
            bootstrap_key: None,
        };
        let err = duplicate_rotation
            .validate(&params)
            .expect_err("duplicate rotation keys must be rejected");
        assert!(err.to_string().contains("duplicate rotation key"));

        let zero_step_rotation = BfvEvaluationKeyBundle {
            relinearization_key: relinearization_key.clone(),
            rotation_keys: vec![BfvRotationKey {
                rotation_steps: 0,
                zero_refresh: zero_refresh.clone(),
            }],
            bootstrap_key: None,
        };
        let err = zero_step_rotation
            .validate(&params)
            .expect_err("zero-step rotation keys must be rejected");
        assert!(err.to_string().contains("greater than zero"));

        let mut malformed_rotation_key = rotation_key;
        malformed_rotation_key.zero_refresh.c0.pop();
        let malformed_rotation = BfvEvaluationKeyBundle {
            relinearization_key: relinearization_key.clone(),
            rotation_keys: vec![malformed_rotation_key],
            bootstrap_key: None,
        };
        let err = malformed_rotation
            .validate(&params)
            .expect_err("malformed rotation refresh ciphertext must be rejected");
        assert!(err.to_string().contains("ciphertext c0 length"));

        let blank_bootstrap_key = BfvEvaluationKeyBundle {
            relinearization_key: relinearization_key.clone(),
            rotation_keys: Vec::new(),
            bootstrap_key: Some(BfvBootstrapKey {
                key_id: "   ".to_owned(),
                zero_refresh: zero_refresh.clone(),
            }),
        };
        let err = blank_bootstrap_key
            .validate(&params)
            .expect_err("blank bootstrap key ids must be rejected");
        assert!(err.to_string().contains("bootstrap key id"));

        let padded_bootstrap_key = BfvEvaluationKeyBundle {
            relinearization_key: relinearization_key.clone(),
            rotation_keys: Vec::new(),
            bootstrap_key: Some(BfvBootstrapKey {
                key_id: " bootstrap-refresh-key".to_owned(),
                zero_refresh: zero_refresh.clone(),
            }),
        };
        let err = padded_bootstrap_key
            .digest(&params)
            .expect_err("padded bootstrap key ids must not receive digests");
        assert!(err.to_string().contains("canonical"));

        let control_bootstrap_key = BfvBootstrapKey {
            key_id: "bootstrap\nkey".to_owned(),
            zero_refresh: zero_refresh.clone(),
        };
        let err = bootstrap_ciphertext(&params, &control_bootstrap_key, &zero_refresh)
            .expect_err("control bytes in bootstrap key ids must be rejected");
        assert!(err.to_string().contains("printable ASCII"));

        let err = bootstrap_key_from_seed(
            &params,
            &public_key,
            "a".repeat(BFV_BOOTSTRAP_KEY_ID_MAX_BYTES + 1),
            b"bfv-oversized-bootstrap-key-id",
        )
        .expect_err("oversized bootstrap key ids must be rejected");
        assert!(err.to_string().contains("maximum supported length"));

        let oversized_rotation_bundle = BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: (1..=u32::try_from(BFV_EVALUATION_KEY_MAX_ROTATION_KEYS + 1)
                .expect("rotation-key count fits into u32"))
                .map(|rotation_steps| BfvRotationKey {
                    rotation_steps,
                    zero_refresh: zero_refresh.clone(),
                })
                .collect(),
            bootstrap_key: None,
        };
        let err = oversized_rotation_bundle
            .validate(&params)
            .expect_err("oversized rotation-key bundles must be rejected");
        assert!(err.to_string().contains("rotation keys"));
    }

    #[cfg(feature = "bfv-accel")]
    #[test]
    fn add_mod_prime_handles_large_modulus_without_overflow() {
        let modulus = u64::MAX - 58;
        assert_eq!(add_mod_prime(modulus - 2, 10, modulus), 8);
        assert_eq!(add_mod_prime(u64::MAX, u64::MAX, modulus), 116);
        assert_eq!(add_mod_prime(1, 1, 0), 0);
    }

    #[cfg(feature = "bfv-accel")]
    #[test]
    fn sub_mod_prime_handles_large_modulus_without_overflow() {
        let modulus = u64::MAX - 58;
        let lhs = 7;
        let rhs = modulus - 11;
        assert_eq!(sub_mod_prime(lhs, rhs, modulus), 18);
        assert_eq!(sub_mod_prime(1, 1, 0), 0);
    }

    #[cfg(feature = "bfv-accel")]
    #[test]
    fn sub_mod_prime_reduces_unbounded_rhs_before_subtracting() {
        let modulus = 17;
        let lhs = 3;
        let rhs = 41;
        assert_eq!(sub_mod_prime(lhs, rhs, modulus), 13);
    }

    #[test]
    fn scalar_modular_helpers_handle_max_width_values() {
        let modulus = u64::MAX;
        assert_eq!(add_mod_u64(modulus - 1, modulus - 2, modulus), modulus - 3);
        assert_eq!(mul_mod_u64(modulus - 1, modulus - 2, modulus), 2);
        assert_eq!(sub_mod_u64(1, modulus - 1, modulus), 2);
        assert_eq!(mod_q(-1, modulus), modulus - 1);
    }

    #[test]
    fn identifier_envelope_roundtrip() {
        let params = ram_lfe_bfv_parameters_v1();
        let (public_parameters, secret_key, _) = derive_identifier_key_material_from_seed(
            &params,
            63,
            b"identifier-envelope-seed",
            b"phone#retail",
        )
        .expect("derive identifier key material");
        let ciphertext = encrypt_identifier_from_seed(
            &public_parameters,
            b"+15551234567",
            b"identifier-envelope-ciphertext",
        )
        .expect("encrypt identifier");
        let plaintext =
            decrypt_identifier(&public_parameters, &secret_key, &ciphertext).expect("decrypt");
        assert_eq!(plaintext, b"+15551234567");
    }

    #[test]
    fn identifier_slot_encoding_and_seed_derivation_are_deterministic() {
        let public_parameters = BfvIdentifierPublicParameters {
            parameters: ram_lfe_bfv_parameters_v1(),
            public_key: BfvPublicKey {
                b: Vec::new(),
                a: Vec::new(),
            },
            max_input_bytes: 3,
        };

        let slots = encode_identifier_slots(&public_parameters, b"abc")
            .expect("identifier slots should encode");
        assert_eq!(
            slots,
            vec![3, u64::from(b'a'), u64::from(b'b'), u64::from(b'c')]
        );

        let first = derive_identifier_slot_seed(b"seed", 1).expect("slot seed should derive");
        let second = derive_identifier_slot_seed(b"seed", 1).expect("slot seed should repeat");
        let other = derive_identifier_slot_seed(b"seed", 2).expect("other slot seed should derive");
        assert_eq!(first, second);
        assert_ne!(first, other);
    }

    #[test]
    fn identifier_public_parameters_reject_unregistered_bfv_profile() {
        let params = sample_identifier_parameters();
        params
            .validate()
            .expect("sample profile is structurally valid");
        assert_ne!(params, ram_lfe_bfv_parameters_v1());
        let (_, public_key, _) =
            keygen_from_seed(&params, b"bfv-unregistered-identifier-keygen").expect("keygen");
        let public_parameters = BfvIdentifierPublicParameters {
            parameters: params,
            public_key,
            max_input_bytes: 63,
        };

        let err = public_parameters
            .validate()
            .expect_err("identifier public parameters must use a registered BFV profile");
        assert!(err.to_string().contains("not registered"));

        let err =
            encrypt_identifier_from_seed(&public_parameters, b"abc", b"bfv-unregistered-input")
                .expect_err("identifier encryption must reject unregistered BFV profiles");
        assert!(err.to_string().contains("not registered"));
    }

    #[test]
    fn identifier_envelope_decryption_rejects_adversarial_plaintext_metadata() {
        let params = ram_lfe_bfv_parameters_v1();
        let (public_parameters, secret_key, _) = derive_identifier_key_material_from_seed(
            &params,
            3,
            b"identifier-envelope-negative-seed",
            b"email#retail",
        )
        .expect("derive identifier key material");
        let mut ciphertext =
            encrypt_identifier_from_seed(&public_parameters, b"a", b"identifier-envelope-valid")
                .expect("encrypt identifier");

        let mut declared_too_long = ciphertext.clone();
        declared_too_long.slots[0] = encrypt_from_seed(
            &params,
            &public_parameters.public_key,
            &[4],
            b"identifier-envelope-long-len",
        )
        .expect("encrypt adversarial length");
        let err = decrypt_identifier(&public_parameters, &secret_key, &declared_too_long)
            .expect_err("declared length beyond max_input_bytes must be rejected");
        assert!(err.to_string().contains("exceeds max_input_bytes"));

        let mut non_zero_trailing = ciphertext.clone();
        non_zero_trailing.slots[2] = encrypt_from_seed(
            &params,
            &public_parameters.public_key,
            &[1],
            b"identifier-envelope-trailing",
        )
        .expect("encrypt adversarial trailing slot");
        let err = decrypt_identifier(&public_parameters, &secret_key, &non_zero_trailing)
            .expect_err("non-zero trailing slots must be rejected");
        assert!(err.to_string().contains("non-zero trailing slots"));

        let mut byte_out_of_range = ciphertext.clone();
        byte_out_of_range.slots[1] = encrypt_from_seed(
            &params,
            &public_parameters.public_key,
            &[u64::from(u8::MAX) + 1],
            b"identifier-envelope-byte-out-of-range",
        )
        .expect("encrypt adversarial byte slot");
        let err = decrypt_identifier(&public_parameters, &secret_key, &byte_out_of_range)
            .expect_err("byte slots outside u8 must be rejected");
        assert!(err.to_string().contains("does not fit into u8"));

        ciphertext.slots[0].c1.pop();
        let err = decrypt_identifier(&public_parameters, &secret_key, &ciphertext)
            .expect_err("malformed slot ciphertext shape must be rejected");
        assert!(err.to_string().contains("ciphertext c1 length"));
    }

    #[test]
    fn homomorphic_plain_multiplication_matches_plaintext() {
        let params = params();
        let (secret_key, public_key, _) =
            keygen_from_seed(&params, b"bfv-plain-mul-keygen").expect("keygen");
        let ciphertext =
            encrypt_from_seed(&params, &public_key, &[11], b"bfv-plain-mul-input").expect("enc");
        let product = multiply_plain_scalar(&params, &ciphertext, 9).expect("mul plain");
        let plaintext = decrypt(&params, &secret_key, &product).expect("decrypt");
        assert_eq!(plaintext[0], 99);
    }

    #[test]
    fn homomorphic_ciphertext_multiplication_matches_plaintext() {
        let params = params();
        let (secret_key, public_key, relin_key) =
            keygen_from_seed(&params, b"bfv-ct-mul-keygen").expect("keygen");
        let lhs =
            encrypt_from_seed(&params, &public_key, &[7], b"bfv-ct-mul-left").expect("enc lhs");
        let rhs =
            encrypt_from_seed(&params, &public_key, &[9], b"bfv-ct-mul-right").expect("enc rhs");
        let encoded_product = poly_mul_mod(
            &params,
            &encode_plaintext(&params, &[7]),
            &encode_plaintext(&params, &[9]),
        );
        let encoded_plaintext = decode_plaintext(&params, &encoded_product);
        assert_eq!(encoded_plaintext[0], 63);
        let raw_c0 = poly_mul_mod(&params, &lhs.c0, &rhs.c0);
        let raw_c1 = poly_add_mod(
            &params,
            &poly_mul_mod(&params, &lhs.c0, &rhs.c1),
            &poly_mul_mod(&params, &lhs.c1, &rhs.c0),
        );
        let raw_c2 = poly_mul_mod(&params, &lhs.c1, &rhs.c1);
        let raw_scaled = poly_add_mod(
            &params,
            &raw_c0,
            &poly_add_mod(
                &params,
                &poly_mul_mod(&params, &raw_c1, &secret_key.s),
                &poly_mul_mod(
                    &params,
                    &raw_c2,
                    &poly_mul_mod(&params, &secret_key.s, &secret_key.s),
                ),
            ),
        );
        let raw_plaintext = decode_plaintext(&params, &raw_scaled);
        assert_eq!(raw_plaintext[0], 63);
        let product = multiply_ciphertexts(&params, &relin_key, &lhs, &rhs).expect("multiply");
        let plaintext = decrypt(&params, &secret_key, &product).expect("decrypt");
        assert_eq!(plaintext[0], 63);
    }

    #[test]
    fn ram_lfe_zero_ciphertext_powers_remain_zero() {
        let params = ram_lfe_bfv_parameters_v1();
        let (secret_key, public_key, relin_key) =
            keygen_from_seed(&params, b"bfv-ram-zero-powers-keygen").expect("keygen");
        let mut value = encrypt_from_seed(&params, &public_key, &[0], b"bfv-ram-zero-powers-input")
            .expect("encrypt zero");
        for round in 0..10 {
            value = multiply_ciphertexts(&params, &relin_key, &value, &value).expect("square");
            let plaintext = decrypt(&params, &secret_key, &value).expect("decrypt");
            assert_eq!(plaintext[0], 0, "round {round}");
        }
    }

    #[test]
    fn affine_circuit_matches_plaintext() {
        let params = params();
        let (secret_key, public_key, _) =
            keygen_from_seed(&params, b"bfv-affine-keygen").expect("keygen");
        let inputs = vec![
            encrypt_from_seed(&params, &public_key, &[5], b"bfv-affine-input-a")
                .expect("encrypt input a"),
            encrypt_from_seed(&params, &public_key, &[11], b"bfv-affine-input-b")
                .expect("encrypt input b"),
        ];
        let circuit = BfvAffineCircuit {
            weights: vec![vec![3, 4], vec![7, 2]],
            bias: vec![9, 1],
        };
        let outputs = evaluate_affine_circuit(&params, &circuit, &inputs).expect("evaluate");
        let first = decrypt(&params, &secret_key, &outputs[0]).expect("decrypt first");
        let second = decrypt(&params, &secret_key, &outputs[1]).expect("decrypt second");
        assert_eq!(first[0], 68);
        assert_eq!(second[0], 58);
    }

    #[test]
    fn reports_selected_convolution_backend() {
        let params = params();
        #[cfg(feature = "bfv-accel")]
        assert_eq!(params.convolution_backend(), BfvConvolutionBackend::CrtNtt);
        #[cfg(not(feature = "bfv-accel"))]
        assert_eq!(
            params.convolution_backend(),
            BfvConvolutionBackend::ScalarSchoolbook
        );
    }

    #[test]
    fn convolution_backend_handles_zero_degree_without_panic() {
        let params = BfvParameters {
            polynomial_degree: 0,
            ciphertext_modulus: 257 * (1_u64 << 20),
            plaintext_modulus: 257,
            decomposition_base_log: 8,
        };
        assert_eq!(
            params.convolution_backend(),
            BfvConvolutionBackend::ScalarSchoolbook
        );
    }

    #[cfg(feature = "bfv-accel")]
    #[test]
    fn crt_ntt_helpers_reject_invalid_lengths_without_panic() {
        let params = sample_identifier_parameters();
        let lhs = vec![1; params.degree() - 1];
        let rhs = vec![1; params.degree()];
        assert_eq!(try_poly_mul_raw_crt_ntt(&params, &lhs, &rhs), None);
        assert_eq!(
            poly_mul_raw_crt_ntt(&params, &lhs, &rhs),
            poly_mul_raw_scalar(&params, &lhs, &rhs)
        );

        assert_eq!(convolve_linear_crt_ntt(&[], &[]), None);
        assert_eq!(convolve_linear_crt_ntt(&[1, 2, 3], &[4, 5, 6]), None);
        assert_eq!(root_for_length(CRT_NTT_PRIMES[0], 0), None);
        assert_eq!(root_for_length(CRT_NTT_PRIMES[0], 3), None);
        assert_eq!(root_for_length(CRT_NTT_PRIMES[1], 1_usize << 17), None);
    }

    #[cfg(feature = "bfv-accel")]
    #[test]
    fn crt_reconstruction_overflow_returns_none() {
        let wide_prime = NttPrime {
            modulus: u64::MAX,
            primitive_root: 2,
            max_power_of_two: 1,
        };
        assert_eq!(
            garner_reconstruct_u128(
                &[0, 0, 0, 0],
                &[wide_prime, wide_prime, wide_prime, wide_prime]
            ),
            None
        );
    }

    #[cfg(feature = "bfv-accel")]
    #[test]
    fn crt_ntt_negacyclic_product_matches_scalar_baseline() {
        let params = sample_identifier_parameters();
        let lhs = sample_uniform_poly(&params, &mut derive_rng(b"bfv-crt-ntt-lhs", b"lhs"));
        let rhs = sample_uniform_poly(&params, &mut derive_rng(b"bfv-crt-ntt-rhs", b"rhs"));
        assert_eq!(
            poly_mul_raw_crt_ntt(&params, &lhs, &rhs),
            poly_mul_raw_scalar(&params, &lhs, &rhs)
        );
    }

    #[test]
    fn rejects_parameter_sets_that_overflow_scalar_accumulators() {
        let params = BfvParameters {
            polynomial_degree: 32_768,
            ciphertext_modulus: 1_u64 << 63,
            plaintext_modulus: 256,
            decomposition_base_log: 8,
        };
        assert_eq!(
            params.validate(),
            Err(BfvError::InvalidParameters(
                "parameter set exceeds the deterministic BFV exact-arithmetic overflow bounds"
                    .to_owned(),
            ))
        );
    }

    #[test]
    fn registered_bfv_parameters_reject_structural_but_unregistered_sets() {
        let mut params = ram_lfe_bfv_parameters_v1();
        params.decomposition_base_log = params.decomposition_base_log.saturating_add(1);
        params
            .validate()
            .expect("adversarial parameter set remains structurally valid");

        let err = validate_registered_bfv_parameters(&params)
            .expect_err("production paths must reject unregistered BFV parameter sets");
        assert!(err.to_string().contains("not registered"));

        let err = registered_bfv_parameter_digest(&params)
            .expect_err("unregistered BFV parameter sets must not receive production digests");
        assert!(err.to_string().contains("not registered"));
    }

    fn sample_identifier_parameters() -> BfvParameters {
        BfvParameters {
            polynomial_degree: 64,
            ciphertext_modulus: 1_u64 << 40,
            plaintext_modulus: 256,
            decomposition_base_log: 12,
        }
    }
}
