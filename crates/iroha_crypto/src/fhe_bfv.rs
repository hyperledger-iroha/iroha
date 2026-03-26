//! Deterministic BFV (Brakerski/Fan-Vercauteren) baseline for homomorphic
//! scalar evaluation.
//!
//! This module implements the textbook integer-arithmetic BFV scheme over the
//! negacyclic ring `Z_q[x] / (x^n + 1)` with:
//! - seeded key generation,
//! - public-key encryption / secret-key decryption,
//! - ciphertext addition,
//! - ciphertext-by-plaintext multiplication,
//! - ciphertext-by-ciphertext multiplication with relinearization,
//! - and a compact affine-circuit evaluator over scalar ciphertext inputs.
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
        #[cfg(feature = "bfv-accel")]
        {
            let required_log = self
                .degree()
                .checked_mul(2)
                .expect("degree overflow")
                .ilog2();
            if self
                .degree()
                .checked_mul(2)
                .expect("degree overflow")
                .is_power_of_two()
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

    fn delta(&self) -> u64 {
        self.ciphertext_modulus / self.plaintext_modulus
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
    /// Returns [`BfvError`] when the envelope is internally inconsistent.
    pub fn validate(&self) -> Result<(), BfvError> {
        self.parameters.validate()?;
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
    let e = sample_small_poly(params, &mut rng);
    let as_product = poly_mul_mod(params, &a, &secret);
    let b = poly_sub_mod(params, &poly_neg_mod(params, &as_product), &e);

    let secret_sq = poly_mul_mod(params, &secret, &secret);
    let digits = params.decomposition_digits();
    let base = params.decomposition_base();
    let mut scale = 1_u64;
    let mut relin_entries = Vec::with_capacity(digits);
    for _ in 0..digits {
        let relin_a = sample_uniform_poly(params, &mut rng);
        let relin_e = sample_small_poly(params, &mut rng);
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
    let e1 = sample_small_poly(params, &mut rng);
    let e2 = sample_small_poly(params, &mut rng);
    let c0 = poly_add_mod(
        params,
        &poly_add_mod(params, &poly_mul_mod(params, &public_key.b, &u), &e1),
        &encoded_plaintext,
    );
    let c1 = poly_add_mod(params, &poly_mul_mod(params, &public_key.a, &u), &e2);
    Ok(BfvCiphertext { c0, c1 })
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
    encoded[0] = mul_mod_u64(scalar, params.delta(), params.ciphertext_modulus);
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

    let raw_c0 = scale_after_multiplication(params, &poly_mul_raw(params, &lhs.c0, &rhs.c0));
    let raw_c1 = scale_after_multiplication(
        params,
        &poly_add_raw(
            &poly_mul_raw(params, &lhs.c0, &rhs.c1),
            &poly_mul_raw(params, &lhs.c1, &rhs.c0),
        ),
    );
    let raw_c2 = scale_after_multiplication(params, &poly_mul_raw(params, &lhs.c1, &rhs.c1));
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
            let slot_seed = derive_identifier_slot_seed(seed, index);
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
    slots[0] = u64::try_from(input.len()).expect("identifier byte length fits into u64");
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

fn derive_identifier_slot_seed(seed: &[u8], index: usize) -> [u8; Hash::LENGTH] {
    Hash::new(
        [
            IDENTIFIER_SLOT_ENCRYPT_DOMAIN,
            seed,
            &u64::try_from(index)
                .expect("slot index fits into u64")
                .to_le_bytes(),
        ]
        .concat(),
    )
    .into()
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

fn sample_uniform_poly(params: &BfvParameters, rng: &mut ChaCha20Rng) -> Polynomial {
    (0..params.degree())
        .map(|_| rng.random_range(0..params.ciphertext_modulus))
        .collect()
}

fn encode_plaintext(params: &BfvParameters, plaintext: &[u64]) -> Polynomial {
    let mut encoded = zero_poly(params);
    for (slot, &coefficient) in plaintext.iter().enumerate() {
        encoded[slot] = mul_mod_u64(coefficient, params.delta(), params.ciphertext_modulus);
    }
    encoded
}

fn decode_plaintext(params: &BfvParameters, scaled: &[u64]) -> Vec<u64> {
    scaled
        .iter()
        .map(|&coefficient| {
            let centered = center_lift(coefficient, params.ciphertext_modulus);
            mod_t(
                round_ratio_signed(
                    centered,
                    params.plaintext_modulus,
                    params.ciphertext_modulus,
                ),
                params.plaintext_modulus,
            )
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

fn scale_after_multiplication(params: &BfvParameters, poly: &[i128]) -> Polynomial {
    poly.iter()
        .map(|&coefficient| {
            mod_q(
                round_ratio_signed(
                    coefficient,
                    params.plaintext_modulus,
                    params.ciphertext_modulus,
                ),
                params.ciphertext_modulus,
            )
        })
        .collect()
}

fn poly_add_raw(lhs: &[i128], rhs: &[i128]) -> Vec<i128> {
    lhs.iter()
        .zip(rhs)
        .map(|(&left, &right)| left + right)
        .collect()
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
    let n = params.degree();
    let mut acc = vec![0_i128; n];
    for (i, &left) in lhs.iter().enumerate() {
        for (j, &right) in rhs.iter().enumerate() {
            let index = i + j;
            let term = i128::from(left) * i128::from(right);
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
    let n = params.degree();
    let linear = convolve_linear_crt_ntt(lhs, rhs);
    let mut folded = vec![0_i128; n];
    for (index, slot) in folded.iter_mut().enumerate() {
        let low = i128::try_from(linear[index]).expect("linear coefficient fits into i128");
        let high = i128::try_from(linear[index + n]).expect("linear coefficient fits into i128");
        *slot = low - high;
    }
    folded
}

#[cfg(feature = "bfv-accel")]
fn convolve_linear_crt_ntt(lhs: &[u64], rhs: &[u64]) -> Vec<u128> {
    let len = lhs
        .len()
        .checked_mul(2)
        .expect("NTT convolution length overflow");
    let mut residues = Vec::with_capacity(CRT_NTT_PRIMES.len());
    for prime in CRT_NTT_PRIMES {
        residues.push(convolve_linear_mod_prime(lhs, rhs, len, prime));
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
        .collect()
}

#[cfg(feature = "bfv-accel")]
fn convolve_linear_mod_prime(lhs: &[u64], rhs: &[u64], len: usize, prime: NttPrime) -> Vec<u64> {
    let modulus = prime.modulus;
    let mut lhs_ntt = vec![0_u64; len];
    let mut rhs_ntt = vec![0_u64; len];
    for (slot, &coefficient) in lhs_ntt.iter_mut().zip(lhs) {
        *slot = coefficient % modulus;
    }
    for (slot, &coefficient) in rhs_ntt.iter_mut().zip(rhs) {
        *slot = coefficient % modulus;
    }
    ntt_in_place(&mut lhs_ntt, prime, false);
    ntt_in_place(&mut rhs_ntt, prime, false);
    for (left, right) in lhs_ntt.iter_mut().zip(&rhs_ntt) {
        *left = mul_mod_prime(*left, *right, modulus);
    }
    ntt_in_place(&mut lhs_ntt, prime, true);
    lhs_ntt
}

#[cfg(feature = "bfv-accel")]
fn ntt_in_place(values: &mut [u64], prime: NttPrime, invert: bool) {
    bit_reverse_permute(values);
    let len = values.len();
    let modulus = prime.modulus;
    let root = root_for_length(prime, len);
    let root = if invert {
        mod_inv_prime(root, modulus)
    } else {
        root
    };

    let mut stage_len = 2_usize;
    while stage_len <= len {
        let step = mod_pow_prime(root, (len / stage_len) as u64, modulus);
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
        stage_len <<= 1;
    }

    if invert {
        let inv_len = mod_inv_prime(
            u64::try_from(len).expect("NTT length fits into u64"),
            modulus,
        );
        for value in values {
            *value = mul_mod_prime(*value, inv_len, modulus);
        }
    }
}

#[cfg(feature = "bfv-accel")]
fn root_for_length(prime: NttPrime, len: usize) -> u64 {
    let log_len = len.ilog2();
    assert!(
        len.is_power_of_two() && log_len <= prime.max_power_of_two,
        "unsupported NTT length {len} for modulus {}",
        prime.modulus
    );
    mod_pow_prime(
        prime.primitive_root,
        (prime.modulus - 1) / u64::try_from(len).expect("NTT length fits into u64"),
        prime.modulus,
    )
}

#[cfg(feature = "bfv-accel")]
fn bit_reverse_permute(values: &mut [u64]) {
    let bits = values.len().ilog2();
    for index in 0..values.len() {
        let reversed = index.reverse_bits() >> (usize::BITS - bits);
        if reversed > index {
            values.swap(index, reversed);
        }
    }
}

#[cfg(feature = "bfv-accel")]
fn garner_reconstruct_u128(residues: &[u64], primes: &[NttPrime]) -> u128 {
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
        value = value
            .checked_add(u128::from(*coefficient) * weight)
            .expect("CRT reconstruction fits into u128");
        if index + 1 != mixed.len() {
            weight = weight
                .checked_mul(u128::from(primes[index].modulus))
                .expect("CRT basis fits into u128");
        }
    }
    value
}

#[cfg(feature = "bfv-accel")]
fn add_mod_prime(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    let sum = lhs + rhs;
    if sum >= modulus || sum < lhs {
        sum.wrapping_sub(modulus)
    } else {
        sum
    }
}

#[cfg(feature = "bfv-accel")]
fn sub_mod_prime(lhs: u64, rhs: u64, modulus: u64) -> u64 {
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
    u64::try_from((u128::from(lhs) * u128::from(rhs)) % u128::from(modulus))
        .expect("reduced prime-field product fits into u64")
}

#[cfg(feature = "bfv-accel")]
fn mod_pow_prime(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
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
    mod_pow_prime(value, modulus - 2, modulus)
}

fn add_mod_u64(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    u64::try_from((u128::from(lhs) + u128::from(rhs)) % u128::from(modulus))
        .expect("reduced ciphertext sum fits into u64")
}

fn sub_mod_u64(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    mod_q(i128::from(lhs) - i128::from(rhs), modulus)
}

fn mul_mod_u64(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    u64::try_from((u128::from(lhs) * u128::from(rhs)) % u128::from(modulus))
        .expect("reduced ciphertext product fits into u64")
}

fn mod_q(value: i128, modulus: u64) -> u64 {
    let modulus = i128::from(modulus);
    let reduced = value.rem_euclid(modulus);
    u64::try_from(reduced).expect("reduced coefficient fits into u64")
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

fn round_ratio_signed(value: i128, numerator: u64, denominator: u64) -> i128 {
    let numerator = i128::from(numerator);
    let denominator = i128::from(denominator);
    let scaled = value * numerator;
    if scaled >= 0 {
        (scaled + denominator / 2) / denominator
    } else {
        (scaled - denominator / 2) / denominator
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

    #[cfg(feature = "bfv-accel")]
    #[test]
    fn sub_mod_prime_handles_large_modulus_without_overflow() {
        let modulus = u64::MAX - 58;
        let lhs = 7;
        let rhs = modulus - 11;
        assert_eq!(sub_mod_prime(lhs, rhs, modulus), 18);
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
    fn identifier_envelope_roundtrip() {
        let params = sample_identifier_parameters();
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
        let encoded_product = scale_after_multiplication(
            &params,
            &poly_mul_raw(
                &params,
                &encode_plaintext(&params, &[7]),
                &encode_plaintext(&params, &[9]),
            ),
        );
        let encoded_plaintext = decode_plaintext(&params, &encoded_product);
        assert_eq!(encoded_plaintext[0], 63);
        let raw_c0 = scale_after_multiplication(&params, &poly_mul_raw(&params, &lhs.c0, &rhs.c0));
        let raw_c1 = scale_after_multiplication(
            &params,
            &poly_add_raw(
                &poly_mul_raw(&params, &lhs.c0, &rhs.c1),
                &poly_mul_raw(&params, &lhs.c1, &rhs.c0),
            ),
        );
        let raw_c2 = scale_after_multiplication(&params, &poly_mul_raw(&params, &lhs.c1, &rhs.c1));
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

    fn sample_identifier_parameters() -> BfvParameters {
        BfvParameters {
            polynomial_degree: 64,
            ciphertext_modulus: 1_u64 << 40,
            plaintext_modulus: 256,
            decomposition_base_log: 12,
        }
    }
}
