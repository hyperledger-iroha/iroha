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
//! The registered first-release RNS chain is currently a representation and
//! release-vector corridor: it covers the ciphertext modulus for coefficient
//! storage and fixture arithmetic, while separate guarded exact-q operations
//! keep product-ring arithmetic from being used as a ciphertext-modulus
//! evaluator unless the caller supplies a sufficiently wide chain. The full
//! BFV-RNS evaluator and basis-extension key-switching path is still pending.

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
const GALOIS_KEYGEN_DOMAIN: &[u8] = b"iroha.crypto.fhe.bfv.galois_keygen.v1";
const IDENTIFIER_KEYGEN_DOMAIN: &[u8] = b"iroha.crypto.fhe.bfv.identifier.keygen.v1";
const IDENTIFIER_SLOT_ENCRYPT_DOMAIN: &[u8] = b"iroha.crypto.fhe.bfv.identifier.slot.v1";
const BFV_PARAMETER_DIGEST_DOMAIN: &[u8] = b"iroha.crypto.fhe.bfv.parameter_digest.v1";
const BFV_EVALUATION_KEY_DIGEST_DOMAIN: &[u8] = b"iroha.crypto.fhe.bfv.eval_key_digest.v1";
const BFV_RNS_MODULUS_CHAIN_DIGEST_DOMAIN: &[u8] =
    b"iroha.crypto.fhe.bfv.rns_modulus_chain_digest.v1";
const BFV_BOOTSTRAP_KEY_ID_MAX_BYTES: usize = 128;
const BFV_BOOTSTRAP_KEY_DEFAULT_MAX_REFRESH_ROUNDS: u16 = 1;
const BFV_BOOTSTRAP_KEY_MAX_REFRESH_ROUNDS: u16 = 1_024;
const BFV_EVALUATION_KEY_MAX_ROTATION_KEYS: usize = 64;
const BFV_EVALUATION_KEY_MAX_GALOIS_KEYS: usize = 64;
const BFV_RNS_MODULUS_CHAIN_MAX_LIMBS: usize = 8;

/// Registered RAM-LFE BFV plaintext modulus.
///
/// RAM-LFE byte predicates evaluate `eq0(x) = 1 - x^256`; this requires a
/// prime plaintext field that contains every byte value as a field element.
pub const RAM_LFE_BFV_PLAINTEXT_MODULUS: u64 = 257;

/// Registered RAM-LFE BFV ciphertext modulus.
pub const RAM_LFE_BFV_CIPHERTEXT_MODULUS: u64 = RAM_LFE_BFV_PLAINTEXT_MODULUS * (1_u64 << 48);

/// Registered RAM-LFE BFV RNS coefficient-modulus chain.
///
/// The limbs are odd primes, strictly increasing, pairwise coprime, and
/// congruent to `1 mod 2n` for the registered `n = 64` RAM-LFE profile so NTT
/// roots exist for negacyclic multiplication. Their product covers the current
/// exact-lift ciphertext modulus while remaining inside the first-release
/// deterministic scalar overflow bound.
pub const RAM_LFE_BFV_RNS_MODULI_V1: [u64; 3] = [358_273, 448_769, 449_921];

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

/// BFV RNS coefficient-modulus chain.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct BfvRnsModulusChain {
    /// Ordered RNS coefficient-modulus limbs.
    pub moduli: Vec<u64>,
}

impl BfvRnsModulusChain {
    /// Validate this RNS chain against a BFV parameter set.
    ///
    /// # Errors
    /// Returns [`BfvError`] when the parameter set or modulus chain is
    /// malformed, noncanonical, unsupported for NTT evaluation, or too small to
    /// cover the parameter-set ciphertext modulus.
    pub fn validate_for_parameters(&self, params: &BfvParameters) -> Result<(), BfvError> {
        params.validate()?;
        validate_bfv_rns_modulus_chain(self, params).map(|_| ())
    }

    /// Validate that this RNS chain also fits the current exact-lift fallback.
    ///
    /// # Errors
    /// Returns [`BfvError`] when the chain is not a valid RNS chain for the
    /// parameter set, or when its product exceeds the deterministic
    /// exact-arithmetic bounds used before the full BFV-RNS engine lands.
    pub fn validate_exact_lift_compatibility(
        &self,
        params: &BfvParameters,
    ) -> Result<(), BfvError> {
        let product = validate_bfv_rns_modulus_chain(self, params)?;
        validate_rns_exact_lift_product_bound(params, product)
    }

    /// Validate that the chain is wide enough for naive exact-lift
    /// ciphertext-modulus addition.
    ///
    /// This is a guard for temporary scalar/RNS fallback code, not a
    /// requirement for the future full BFV-RNS evaluator. A product-ring RNS
    /// addition can be reduced back into `Z_q` without ambiguity only when the
    /// chain product covers every possible unreduced sum of two `Z_q`
    /// coefficients.
    ///
    /// # Errors
    /// Returns [`BfvError`] when the chain is malformed or too narrow for this
    /// exact fallback.
    pub fn validate_exact_ciphertext_modulus_addition_coverage(
        &self,
        params: &BfvParameters,
    ) -> Result<(), BfvError> {
        let product = validate_bfv_rns_modulus_chain(self, params)?;
        let required = exact_ciphertext_modulus_addition_rns_bound(params)?;
        if product < required {
            return Err(BfvError::InvalidParameters(format!(
                "BFV RNS modulus-chain product {product} does not cover exact ciphertext-modulus addition bound {required}"
            )));
        }
        Ok(())
    }

    /// Validate that the chain is wide enough for naive exact-lift
    /// ciphertext-modulus negacyclic multiplication.
    ///
    /// This guard rejects using product-ring RNS multiplication as a drop-in
    /// replacement for `Z_q[x] / (x^n + 1)` multiplication unless the chain can
    /// uniquely represent the signed unreduced coefficient range. The current
    /// registered RAM-LFE chain intentionally does not satisfy this stronger
    /// bound; it remains a representation/vector corridor until the full
    /// BFV-RNS evaluator lands.
    ///
    /// # Errors
    /// Returns [`BfvError`] when the chain is malformed or too narrow for this
    /// exact fallback.
    pub fn validate_exact_ciphertext_modulus_negacyclic_product_coverage(
        &self,
        params: &BfvParameters,
    ) -> Result<(), BfvError> {
        let product = validate_bfv_rns_modulus_chain(self, params)?;
        let required = exact_ciphertext_modulus_negacyclic_product_rns_bound(params)?;
        if product < required {
            return Err(BfvError::InvalidParameters(format!(
                "BFV RNS modulus-chain product {product} does not cover exact ciphertext-modulus negacyclic product bound {required}"
            )));
        }
        Ok(())
    }

    /// Return the checked product of all RNS limbs.
    ///
    /// # Errors
    /// Returns [`BfvError`] when the chain is empty or the product does not fit
    /// into `u128`.
    pub fn product(&self) -> Result<u128, BfvError> {
        checked_rns_modulus_product(&self.moduli)
    }

    /// Return a stable digest over this RNS modulus chain.
    ///
    /// # Errors
    /// Returns [`BfvError`] when validation or canonical encoding fails.
    pub fn digest_for_parameters(&self, params: &BfvParameters) -> Result<Hash, BfvError> {
        self.validate_for_parameters(params)?;
        let bytes = norito::to_bytes(self).map_err(|err| {
            BfvError::InvalidParameters(format!("RNS modulus-chain encoding failed: {err}"))
        })?;
        Ok(Hash::new_from_chunks(&[
            BFV_RNS_MODULUS_CHAIN_DIGEST_DOMAIN,
            bytes.as_slice(),
        ]))
    }

    /// Decompose a BFV polynomial into limb-major RNS residues.
    ///
    /// # Errors
    /// Returns [`BfvError`] when the chain, parameter set, or source
    /// polynomial shape is invalid.
    pub fn decompose_polynomial(
        &self,
        params: &BfvParameters,
        coefficients: &[u64],
    ) -> Result<BfvRnsPolynomial, BfvError> {
        self.validate_for_parameters(params)?;
        validate_poly(params, coefficients, "RNS source polynomial")?;
        let residues_by_limb = self
            .moduli
            .iter()
            .map(|&modulus| {
                coefficients
                    .iter()
                    .map(|&coefficient| coefficient % modulus)
                    .collect()
            })
            .collect();
        Ok(BfvRnsPolynomial { residues_by_limb })
    }

    /// Reconstruct coefficient values from limb-major RNS residues.
    ///
    /// # Errors
    /// Returns [`BfvError`] when the chain, parameter set, or residue shape is
    /// invalid, or when CRT reconstruction overflows `u128`.
    pub fn reconstruct_polynomial(
        &self,
        params: &BfvParameters,
        polynomial: &BfvRnsPolynomial,
    ) -> Result<Vec<u128>, BfvError> {
        self.validate_for_parameters(params)?;
        validate_rns_polynomial(params, self, polynomial)?;
        (0..params.degree())
            .map(|index| {
                let residues = polynomial
                    .residues_by_limb
                    .iter()
                    .map(|limb| limb[index])
                    .collect::<Vec<_>>();
                reconstruct_rns_coefficient(&residues, &self.moduli)
            })
            .collect()
    }

    /// Add two RNS polynomials coefficient-wise in the chain product ring.
    ///
    /// # Errors
    /// Returns [`BfvError`] when the chain, parameter set, or either operand
    /// is malformed.
    pub fn add_rns_polynomials(
        &self,
        params: &BfvParameters,
        lhs: &BfvRnsPolynomial,
        rhs: &BfvRnsPolynomial,
    ) -> Result<BfvRnsPolynomial, BfvError> {
        validate_rns_polynomial_pair(params, self, lhs, rhs)?;
        let residues_by_limb = lhs
            .residues_by_limb
            .iter()
            .zip(&rhs.residues_by_limb)
            .zip(&self.moduli)
            .map(|((lhs_limb, rhs_limb), &modulus)| {
                lhs_limb
                    .iter()
                    .zip(rhs_limb)
                    .map(|(&left, &right)| add_mod_u64(left, right, modulus))
                    .collect()
            })
            .collect();
        Ok(BfvRnsPolynomial { residues_by_limb })
    }

    /// Multiply two RNS polynomials in `Z_Q[x] / (x^n + 1)`.
    ///
    /// # Errors
    /// Returns [`BfvError`] when the chain, parameter set, or either operand
    /// is malformed.
    pub fn multiply_rns_polynomials_negacyclic(
        &self,
        params: &BfvParameters,
        lhs: &BfvRnsPolynomial,
        rhs: &BfvRnsPolynomial,
    ) -> Result<BfvRnsPolynomial, BfvError> {
        validate_rns_polynomial_pair(params, self, lhs, rhs)?;
        let residues_by_limb = lhs
            .residues_by_limb
            .iter()
            .zip(&rhs.residues_by_limb)
            .zip(&self.moduli)
            .map(|((lhs_limb, rhs_limb), &modulus)| {
                multiply_rns_limb_negacyclic(params, lhs_limb, rhs_limb, modulus)
            })
            .collect();
        Ok(BfvRnsPolynomial { residues_by_limb })
    }

    /// Add two ciphertext-modulus polynomials through an exact RNS corridor.
    ///
    /// This helper is intentionally guarded by
    /// [`Self::validate_exact_ciphertext_modulus_addition_coverage`]. It is
    /// suitable for exact fallback/evaluator tests whose chain product is wide
    /// enough to reconstruct every unreduced `Z_q` coefficient sum before
    /// reducing back modulo `q`.
    ///
    /// # Errors
    /// Returns [`BfvError`] when the chain is malformed, too narrow for exact
    /// `Z_q` addition, or either polynomial is malformed.
    pub fn add_ciphertext_modulus_polynomials_exact(
        &self,
        params: &BfvParameters,
        lhs: &[u64],
        rhs: &[u64],
    ) -> Result<Vec<u64>, BfvError> {
        self.validate_exact_ciphertext_modulus_addition_coverage(params)?;
        validate_poly(params, lhs, "RNS exact-add lhs polynomial")?;
        validate_poly(params, rhs, "RNS exact-add rhs polynomial")?;

        let lhs_rns = self.decompose_polynomial(params, lhs)?;
        let rhs_rns = self.decompose_polynomial(params, rhs)?;
        let sum_rns = self.add_rns_polynomials(params, &lhs_rns, &rhs_rns)?;
        self.reconstruct_polynomial(params, &sum_rns)?
            .into_iter()
            .map(|coefficient| reduce_u128_to_u64_mod(coefficient, params.ciphertext_modulus))
            .collect()
    }

    /// Multiply two ciphertext-modulus polynomials through an exact RNS corridor.
    ///
    /// This helper is intentionally guarded by
    /// [`Self::validate_exact_ciphertext_modulus_negacyclic_product_coverage`].
    /// Negacyclic products contain signed wraparound terms, so reconstruction
    /// uses the unique centered representative before reducing back modulo
    /// `q`.
    ///
    /// # Errors
    /// Returns [`BfvError`] when the chain is malformed, too narrow for exact
    /// `Z_q` negacyclic multiplication, or either polynomial is malformed.
    pub fn multiply_ciphertext_modulus_polynomials_negacyclic_exact(
        &self,
        params: &BfvParameters,
        lhs: &[u64],
        rhs: &[u64],
    ) -> Result<Vec<u64>, BfvError> {
        self.validate_exact_ciphertext_modulus_negacyclic_product_coverage(params)?;
        validate_poly(params, lhs, "RNS exact-multiply lhs polynomial")?;
        validate_poly(params, rhs, "RNS exact-multiply rhs polynomial")?;

        let chain_product = self.product()?;
        let centered_abs_bound = exact_ciphertext_modulus_negacyclic_product_abs_bound(params)?;
        let lhs_rns = self.decompose_polynomial(params, lhs)?;
        let rhs_rns = self.decompose_polynomial(params, rhs)?;
        let product_rns = self.multiply_rns_polynomials_negacyclic(params, &lhs_rns, &rhs_rns)?;
        self.reconstruct_polynomial(params, &product_rns)?
            .into_iter()
            .map(|coefficient| {
                reduce_centered_rns_value_to_u64_mod(
                    coefficient,
                    chain_product,
                    centered_abs_bound,
                    params.ciphertext_modulus,
                )
            })
            .collect()
    }
}

/// BFV polynomial represented as limb-major RNS residues.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct BfvRnsPolynomial {
    /// Residues grouped by modulus-chain limb, then polynomial coefficient.
    pub residues_by_limb: Vec<Vec<u64>>,
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

/// Key-switching material for one BFV Galois automorphism.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct BfvGaloisKey {
    /// Canonical odd automorphism power `k` for `x -> x^k`.
    pub automorphism_power: u32,
    /// Key-switching entries encrypting `sigma_k(s)` under the original secret.
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
    /// Maximum number of refresh rounds this key authorizes for one job.
    pub max_refresh_rounds: u16,
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
    /// Galois automorphism keys admitted for packed-polynomial key switching.
    #[norito(default)]
    pub galois_keys: Vec<BfvGaloisKey>,
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
        if self.galois_keys.len() > BFV_EVALUATION_KEY_MAX_GALOIS_KEYS {
            return Err(BfvError::InvalidParameters(format!(
                "evaluation-key bundle supports at most {BFV_EVALUATION_KEY_MAX_GALOIS_KEYS} Galois keys"
            )));
        }
        let mut seen_galois = std::collections::BTreeSet::new();
        for key in &self.galois_keys {
            validate_galois_key(params, key)?;
            if !seen_galois.insert(key.automorphism_power) {
                return Err(BfvError::InvalidParameters(format!(
                    "duplicate Galois key for automorphism power {}",
                    key.automorphism_power
                )));
            }
        }
        if let Some(bootstrap_key) = self.bootstrap_key.as_ref() {
            validate_bootstrap_key(params, bootstrap_key)?;
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
        Ok(Hash::new_from_chunks(&[
            BFV_EVALUATION_KEY_DIGEST_DOMAIN,
            bytes.as_slice(),
        ]))
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

/// Return the registered BFV RNS chain for RAM-LFE byte-slot programs.
#[must_use]
pub fn ram_lfe_bfv_rns_modulus_chain_v1() -> BfvRnsModulusChain {
    BfvRnsModulusChain {
        moduli: RAM_LFE_BFV_RNS_MODULI_V1.to_vec(),
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

/// Return the registered BFV RNS chain for a production BFV parameter set.
///
/// # Errors
/// Returns [`BfvError`] when the parameter set is not registered or the
/// registered chain fails validation.
pub fn registered_bfv_rns_modulus_chain(
    params: &BfvParameters,
) -> Result<BfvRnsModulusChain, BfvError> {
    validate_registered_bfv_parameters(params)?;
    let chain = ram_lfe_bfv_rns_modulus_chain_v1();
    chain.validate_exact_lift_compatibility(params)?;
    Ok(chain)
}

/// Return the stable digest for a registered BFV RNS modulus chain.
///
/// # Errors
/// Returns [`BfvError`] when the parameter set is not registered or the
/// registered chain fails validation or canonical encoding.
pub fn registered_bfv_rns_modulus_chain_digest(params: &BfvParameters) -> Result<Hash, BfvError> {
    let chain = registered_bfv_rns_modulus_chain(params)?;
    chain.digest_for_parameters(params)
}

/// Return the stable digest for a registered BFV parameter set.
///
/// # Errors
/// Returns [`BfvError`] when the parameter set is not registered.
pub fn registered_bfv_parameter_digest(params: &BfvParameters) -> Result<Hash, BfvError> {
    validate_registered_bfv_parameters(params)?;
    let bytes = norito::to_bytes(params)
        .map_err(|err| BfvError::InvalidParameters(format!("parameter encoding failed: {err}")))?;
    Ok(Hash::new_from_chunks(&[
        BFV_PARAMETER_DIGEST_DOMAIN,
        bytes.as_slice(),
    ]))
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

/// Derive deterministic key-switching material for a BFV Galois automorphism.
///
/// The key switches ciphertexts encrypted under `sigma_k(s)` back to the
/// original secret key `s`, where `sigma_k` maps `x -> x^k` in
/// `Z_q[x] / (x^n + 1)`. This is the deterministic baseline primitive needed
/// before packed slot rotations can be wired through public Galois keys.
///
/// # Errors
/// Returns [`BfvError`] when parameters, secret-key shape, or automorphism
/// power are invalid.
pub fn galois_key_from_seed(
    params: &BfvParameters,
    secret_key: &BfvSecretKey,
    automorphism_power: u32,
    seed: &[u8],
) -> Result<BfvGaloisKey, BfvError> {
    params.validate()?;
    validate_secret_key(params, secret_key)?;
    validate_galois_automorphism_power(params, automorphism_power)?;

    let target_secret = apply_galois_automorphism_poly(params, &secret_key.s, automorphism_power)?;
    let power_bytes = automorphism_power.to_le_bytes();
    let material: [u8; Hash::LENGTH] =
        Hash::new_from_chunks(&[GALOIS_KEYGEN_DOMAIN, power_bytes.as_slice(), seed]).into();
    let mut rng = ChaCha20Rng::from_seed(material);
    let entries = key_switch_entries_from_rng(params, &secret_key.s, &target_secret, &mut rng);
    let key = BfvGaloisKey {
        automorphism_power,
        entries,
    };
    validate_galois_key(params, &key)?;
    Ok(key)
}

/// Apply a packed-polynomial BFV Galois automorphism and key switch back to `s`.
///
/// The returned ciphertext decrypts under the original secret key to the
/// automorphed plaintext polynomial. The operation is deterministic for a fixed
/// input ciphertext and Galois key and does not require the evaluator to hold
/// the secret key.
///
/// # Errors
/// Returns [`BfvError`] when the ciphertext or Galois key does not match the
/// parameter set.
pub fn apply_galois_automorphism_ciphertext(
    params: &BfvParameters,
    galois_key: &BfvGaloisKey,
    ciphertext: &BfvCiphertext,
) -> Result<BfvCiphertext, BfvError> {
    params.validate()?;
    validate_ciphertext(params, ciphertext)?;
    validate_galois_key(params, galois_key)?;

    let automorphed_c0 =
        apply_galois_automorphism_poly(params, &ciphertext.c0, galois_key.automorphism_power)?;
    let automorphed_c1 =
        apply_galois_automorphism_poly(params, &ciphertext.c1, galois_key.automorphism_power)?;
    Ok(key_switch_from_transformed_secret(
        params,
        &galois_key.entries,
        &automorphed_c0,
        &automorphed_c1,
    ))
}

/// Apply a packed-polynomial BFV Galois automorphism through an exact RNS corridor.
///
/// The ciphertext components are automorphed in the scalar `Z_q` representation,
/// then the transformed secret component is key-switched with guarded exact RNS
/// polynomial products and additions. This is an exact evaluator bridge for
/// sufficiently wide chains, not the final BFV-RNS basis-extension pipeline.
///
/// # Errors
/// Returns [`BfvError`] when the ciphertext or Galois key does not match the
/// parameter set, or when the RNS chain is malformed or too narrow for exact
/// ciphertext-modulus key switching.
pub fn apply_galois_automorphism_ciphertext_rns_exact(
    params: &BfvParameters,
    rns_chain: &BfvRnsModulusChain,
    galois_key: &BfvGaloisKey,
    ciphertext: &BfvCiphertext,
) -> Result<BfvCiphertext, BfvError> {
    params.validate()?;
    validate_ciphertext(params, ciphertext)?;
    validate_galois_key(params, galois_key)?;

    let automorphed_c0 =
        apply_galois_automorphism_poly(params, &ciphertext.c0, galois_key.automorphism_power)?;
    let automorphed_c1 =
        apply_galois_automorphism_poly(params, &ciphertext.c1, galois_key.automorphism_power)?;
    key_switch_from_transformed_secret_rns_exact(
        params,
        rns_chain,
        &galois_key.entries,
        &automorphed_c0,
        &automorphed_c1,
    )
}

/// Encode a full BFV packed-slot plaintext into polynomial coefficients.
///
/// The registered RAM-LFE profile has `t = 257` and `n = 64`, so `t = 1 mod
/// 2n` and the plaintext ring splits into `n` CRT slots. This helper performs
/// deterministic interpolation over those CRT points and returns coefficients
/// in `Z_t[x] / (x^n + 1)`, ready to pass to [`encrypt_from_seed`].
///
/// # Errors
/// Returns [`BfvError`] when the parameter set is not batchable, the slot count
/// is not exactly `polynomial_degree`, or any slot is outside the plaintext
/// modulus.
pub fn encode_packed_plaintext_slots(
    params: &BfvParameters,
    slots: &[u64],
) -> Result<Vec<u64>, BfvError> {
    let points = packed_plaintext_evaluation_points(params)?;
    if slots.len() != params.degree() {
        return Err(BfvError::ShapeMismatch(format!(
            "packed plaintext expected {} slots, found {}",
            params.polynomial_degree,
            slots.len()
        )));
    }
    validate_plaintext(params, slots)?;

    let modulus = params.plaintext_modulus;
    let degree = params.degree();
    let mut coefficients = vec![0_u64; degree];
    for (slot_index, (&slot_value, &point)) in slots.iter().zip(&points).enumerate() {
        let mut basis = vec![1_u64];
        let mut denominator = 1_u64;
        for (other_index, &other_point) in points.iter().enumerate() {
            if other_index == slot_index {
                continue;
            }
            basis = multiply_plaintext_linear_factor(&basis, other_point, modulus);
            denominator = mul_mod_u64(
                denominator,
                sub_mod_u64(point, other_point, modulus),
                modulus,
            );
        }
        let inverse_denominator = mod_inv_prime_u64(denominator, modulus).ok_or_else(|| {
            BfvError::InvalidParameters(
                "packed plaintext interpolation denominator is not invertible".to_owned(),
            )
        })?;
        let scale = mul_mod_u64(slot_value, inverse_denominator, modulus);
        for (coefficient, basis_coefficient) in coefficients.iter_mut().zip(&basis) {
            *coefficient = add_mod_u64(
                *coefficient,
                mul_mod_u64(*basis_coefficient, scale, modulus),
                modulus,
            );
        }
    }
    Ok(coefficients)
}

/// Decode a full BFV packed-slot plaintext from polynomial coefficients.
///
/// This is the inverse of [`encode_packed_plaintext_slots`] for batchable
/// parameter sets. It evaluates the plaintext polynomial at the canonical
/// odd-power CRT points in `Z_t`.
///
/// # Errors
/// Returns [`BfvError`] when the parameter set is not batchable, the plaintext
/// coefficient count is not exactly `polynomial_degree`, or any coefficient is
/// outside the plaintext modulus.
pub fn decode_packed_plaintext_slots(
    params: &BfvParameters,
    plaintext: &[u64],
) -> Result<Vec<u64>, BfvError> {
    let points = packed_plaintext_evaluation_points(params)?;
    if plaintext.len() != params.degree() {
        return Err(BfvError::ShapeMismatch(format!(
            "packed plaintext expected {} coefficients, found {}",
            params.polynomial_degree,
            plaintext.len()
        )));
    }
    validate_plaintext(params, plaintext)?;
    Ok(points
        .iter()
        .map(|&point| evaluate_plaintext_polynomial_mod(plaintext, point, params.plaintext_modulus))
        .collect())
}

/// Return the packed-slot permutation induced by a Galois automorphism.
///
/// For the canonical slot order used by [`encode_packed_plaintext_slots`], the
/// returned vector maps each output slot index to the input slot index that
/// appears there after applying `x -> x^k`.
///
/// # Errors
/// Returns [`BfvError`] when the parameter set is not batchable or
/// `automorphism_power` is not a canonical BFV Galois automorphism power.
pub fn packed_galois_slot_permutation(
    params: &BfvParameters,
    automorphism_power: u32,
) -> Result<Vec<usize>, BfvError> {
    packed_plaintext_root(params)?;
    let power = validate_galois_automorphism_power(params, automorphism_power)?;
    let degree = params.degree();
    let cyclotomic_order = degree.checked_mul(2).ok_or_else(|| {
        BfvError::InvalidParameters(
            "BFV packed-slot cyclotomic order exceeds deterministic bounds".to_owned(),
        )
    })?;
    (0..degree)
        .map(|slot| {
            let exponent = slot
                .checked_mul(2)
                .and_then(|value| value.checked_add(1))
                .and_then(|value| value.checked_mul(power))
                .map(|value| value % cyclotomic_order)
                .ok_or_else(|| {
                    BfvError::InvalidParameters(
                        "BFV packed-slot permutation exponent exceeds deterministic bounds"
                            .to_owned(),
                    )
                })?;
            Ok((exponent - 1) / 2)
        })
        .collect()
}

/// Derive a deterministic one-round public bootstrap refresh key from a BFV public key.
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
    bootstrap_key_with_max_refresh_rounds_from_seed(
        params,
        public_key,
        key_id,
        BFV_BOOTSTRAP_KEY_DEFAULT_MAX_REFRESH_ROUNDS,
        seed,
    )
}

/// Derive a deterministic bounded public bootstrap refresh key from a BFV public key.
///
/// # Errors
/// Returns [`BfvError`] when parameter, public-key, key id, or refresh-round
/// bounds are invalid.
pub fn bootstrap_key_with_max_refresh_rounds_from_seed(
    params: &BfvParameters,
    public_key: &BfvPublicKey,
    key_id: impl Into<String>,
    max_refresh_rounds: u16,
    seed: &[u8],
) -> Result<BfvBootstrapKey, BfvError> {
    let key_id = key_id.into();
    validate_bootstrap_key_metadata(&key_id, max_refresh_rounds)?;
    let zero_refresh = encrypt_from_seed(params, public_key, &[0], seed)?;
    let bootstrap_key = BfvBootstrapKey {
        key_id,
        max_refresh_rounds,
        zero_refresh,
    };
    validate_bootstrap_key(params, &bootstrap_key)?;
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
    validate_bootstrap_key(params, bootstrap_key)?;
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
    rotated.rotate_left(rotation_steps_mod_slot_count(
        rotation_key.rotation_steps,
        slot_count,
    )?);
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

/// Homomorphically add two ciphertexts through an exact RNS corridor.
///
/// This is a guarded evaluator bridge for parameter sets whose RNS chain is
/// wide enough to reconstruct unreduced ciphertext-modulus coefficient sums
/// before reducing back into `Z_q`. The registered RAM-LFE v1 chain is
/// intentionally narrower than this bound and fails closed here until the full
/// BFV-RNS evaluator lands.
///
/// # Errors
/// Returns [`BfvError`] when ciphertext shapes do not match the parameter set
/// or when the RNS chain is malformed or too narrow for exact `Z_q` addition.
pub fn add_ciphertexts_rns_exact(
    params: &BfvParameters,
    rns_chain: &BfvRnsModulusChain,
    lhs: &BfvCiphertext,
    rhs: &BfvCiphertext,
) -> Result<BfvCiphertext, BfvError> {
    params.validate()?;
    validate_ciphertext(params, lhs)?;
    validate_ciphertext(params, rhs)?;
    Ok(BfvCiphertext {
        c0: rns_chain.add_ciphertext_modulus_polynomials_exact(params, &lhs.c0, &rhs.c0)?,
        c1: rns_chain.add_ciphertext_modulus_polynomials_exact(params, &lhs.c1, &rhs.c1)?,
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

/// Multiply two ciphertexts through an exact RNS corridor and relinearize.
///
/// Raw ciphertext products and relinearization-key products are evaluated with
/// guarded exact RNS `Z_q[x] / (x^n + 1)` polynomial operations. This remains
/// an exact fallback/evaluator bridge, not the final BFV-RNS basis-extension
/// and key-switching pipeline.
///
/// # Errors
/// Returns [`BfvError`] when parameters, operand shapes, relinearization key
/// shape, or RNS exact-coverage bounds are invalid.
pub fn multiply_ciphertexts_rns_exact(
    params: &BfvParameters,
    rns_chain: &BfvRnsModulusChain,
    relinearization_key: &BfvRelinearizationKey,
    lhs: &BfvCiphertext,
    rhs: &BfvCiphertext,
) -> Result<BfvCiphertext, BfvError> {
    params.validate()?;
    validate_ciphertext(params, lhs)?;
    validate_ciphertext(params, rhs)?;
    validate_relinearization_key(params, relinearization_key)?;

    let raw_c0 = rns_chain
        .multiply_ciphertext_modulus_polynomials_negacyclic_exact(params, &lhs.c0, &rhs.c0)?;
    let lhs_c0_rhs_c1 = rns_chain
        .multiply_ciphertext_modulus_polynomials_negacyclic_exact(params, &lhs.c0, &rhs.c1)?;
    let lhs_c1_rhs_c0 = rns_chain
        .multiply_ciphertext_modulus_polynomials_negacyclic_exact(params, &lhs.c1, &rhs.c0)?;
    let raw_c1 = rns_chain.add_ciphertext_modulus_polynomials_exact(
        params,
        &lhs_c0_rhs_c1,
        &lhs_c1_rhs_c0,
    )?;
    let raw_c2 = rns_chain
        .multiply_ciphertext_modulus_polynomials_negacyclic_exact(params, &lhs.c1, &rhs.c1)?;
    relinearize_rns_exact(
        params,
        rns_chain,
        relinearization_key,
        &raw_c0,
        &raw_c1,
        &raw_c2,
    )
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
    let derived_seed = Hash::new_from_chunks(&[IDENTIFIER_KEYGEN_DOMAIN, associated_data, seed]);
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
    validate_key_switch_entries(params, &relinearization_key.entries, "relinearization key")
}

fn validate_galois_key(params: &BfvParameters, galois_key: &BfvGaloisKey) -> Result<(), BfvError> {
    validate_galois_automorphism_power(params, galois_key.automorphism_power)?;
    validate_key_switch_entries(params, &galois_key.entries, "Galois key")
}

fn validate_key_switch_entries(
    params: &BfvParameters,
    entries: &[BfvRelinearizationKeyEntry],
    label: &str,
) -> Result<(), BfvError> {
    if entries.len() != params.decomposition_digits() {
        return Err(BfvError::ShapeMismatch(format!(
            "{label} expected {} entries, found {}",
            params.decomposition_digits(),
            entries.len()
        )));
    }
    for (index, entry) in entries.iter().enumerate() {
        validate_poly(params, &entry.b, &format!("{label} b[{index}]"))?;
        validate_poly(params, &entry.a, &format!("{label} a[{index}]"))?;
    }
    Ok(())
}

fn validate_galois_automorphism_power(
    params: &BfvParameters,
    automorphism_power: u32,
) -> Result<usize, BfvError> {
    let cyclotomic_order = params.degree().checked_mul(2).ok_or_else(|| {
        BfvError::InvalidParameters(
            "BFV Galois automorphism order exceeds deterministic bounds".to_owned(),
        )
    })?;
    let power = usize::try_from(automorphism_power).map_err(|_| {
        BfvError::InvalidParameters(
            "BFV Galois automorphism power exceeds platform usize".to_owned(),
        )
    })?;
    if power == 0 || power >= cyclotomic_order {
        return Err(BfvError::InvalidParameters(format!(
            "BFV Galois automorphism power must be in 1..{cyclotomic_order}"
        )));
    }
    let cyclotomic_order_u64 = u64::try_from(cyclotomic_order).map_err(|_| {
        BfvError::InvalidParameters(
            "BFV Galois automorphism order exceeds deterministic bounds".to_owned(),
        )
    })?;
    if gcd_u64(u64::from(automorphism_power), cyclotomic_order_u64) != 1 {
        return Err(BfvError::InvalidParameters(
            "BFV Galois automorphism power must be coprime to 2 * polynomial_degree".to_owned(),
        ));
    }
    Ok(power)
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

fn validate_bootstrap_key(
    params: &BfvParameters,
    bootstrap_key: &BfvBootstrapKey,
) -> Result<(), BfvError> {
    validate_bootstrap_key_metadata(&bootstrap_key.key_id, bootstrap_key.max_refresh_rounds)?;
    validate_ciphertext(params, &bootstrap_key.zero_refresh)
}

fn validate_bootstrap_key_metadata(key_id: &str, max_refresh_rounds: u16) -> Result<(), BfvError> {
    validate_bootstrap_key_id(key_id)?;
    if max_refresh_rounds == 0 {
        return Err(BfvError::InvalidParameters(
            "bootstrap key max_refresh_rounds must be greater than zero".to_owned(),
        ));
    }
    if max_refresh_rounds > BFV_BOOTSTRAP_KEY_MAX_REFRESH_ROUNDS {
        return Err(BfvError::InvalidParameters(format!(
            "bootstrap key max_refresh_rounds exceeds the supported limit {BFV_BOOTSTRAP_KEY_MAX_REFRESH_ROUNDS}"
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

fn validate_bfv_rns_modulus_chain(
    chain: &BfvRnsModulusChain,
    params: &BfvParameters,
) -> Result<u128, BfvError> {
    if chain.moduli.len() > BFV_RNS_MODULUS_CHAIN_MAX_LIMBS {
        return Err(BfvError::InvalidParameters(format!(
            "BFV RNS modulus chain supports at most {BFV_RNS_MODULUS_CHAIN_MAX_LIMBS} limbs"
        )));
    }
    let root_order = u64::from(params.polynomial_degree)
        .checked_mul(2)
        .ok_or_else(|| {
            BfvError::InvalidParameters(
                "BFV RNS root order exceeds deterministic validation bounds".to_owned(),
            )
        })?;
    let mut previous = 0_u64;
    for (index, &modulus) in chain.moduli.iter().enumerate() {
        if modulus <= params.plaintext_modulus {
            return Err(BfvError::InvalidParameters(format!(
                "BFV RNS modulus limb {index} must exceed plaintext modulus {}",
                params.plaintext_modulus
            )));
        }
        if modulus.is_multiple_of(2) {
            return Err(BfvError::InvalidParameters(format!(
                "BFV RNS modulus limb {index} must be odd"
            )));
        }
        if index > 0 && modulus <= previous {
            return Err(BfvError::InvalidParameters(
                "BFV RNS modulus limbs must be strictly increasing".to_owned(),
            ));
        }
        if !is_prime_u64(modulus) {
            return Err(BfvError::InvalidParameters(format!(
                "BFV RNS modulus limb {index} must be prime"
            )));
        }
        if !(modulus - 1).is_multiple_of(root_order) {
            return Err(BfvError::InvalidParameters(format!(
                "BFV RNS modulus limb {index} must be 1 mod {root_order}"
            )));
        }
        if chain.moduli[..index]
            .iter()
            .any(|&prior| gcd_u64(prior, modulus) != 1)
        {
            return Err(BfvError::InvalidParameters(
                "BFV RNS modulus limbs must be pairwise coprime".to_owned(),
            ));
        }
        previous = modulus;
    }

    let product = checked_rns_modulus_product(&chain.moduli)?;
    if product < u128::from(params.ciphertext_modulus) {
        return Err(BfvError::InvalidParameters(format!(
            "BFV RNS modulus-chain product {product} does not cover ciphertext modulus {}",
            params.ciphertext_modulus
        )));
    }
    Ok(product)
}

fn checked_rns_modulus_product(moduli: &[u64]) -> Result<u128, BfvError> {
    if moduli.is_empty() {
        return Err(BfvError::InvalidParameters(
            "BFV RNS modulus chain must not be empty".to_owned(),
        ));
    }
    moduli.iter().try_fold(1_u128, |product, &modulus| {
        product.checked_mul(u128::from(modulus)).ok_or_else(|| {
            BfvError::InvalidParameters("BFV RNS modulus-chain product exceeds u128".to_owned())
        })
    })
}

fn validate_rns_exact_lift_product_bound(
    params: &BfvParameters,
    product: u128,
) -> Result<(), BfvError> {
    let max_raw_coefficient = u128::from(params.polynomial_degree)
        .checked_mul(product)
        .and_then(|value| value.checked_mul(product))
        .ok_or_else(|| {
            BfvError::InvalidParameters(
                "BFV RNS modulus-chain product exceeds exact-arithmetic overflow bounds".to_owned(),
            )
        })?;
    let max_scaled_coefficient = max_raw_coefficient
        .checked_mul(u128::from(params.plaintext_modulus))
        .ok_or_else(|| {
            BfvError::InvalidParameters(
                "BFV RNS modulus-chain product exceeds exact-arithmetic overflow bounds".to_owned(),
            )
        })?;
    if max_scaled_coefficient > i128::MAX as u128 {
        return Err(BfvError::InvalidParameters(
            "BFV RNS modulus-chain product exceeds exact-arithmetic overflow bounds".to_owned(),
        ));
    }
    Ok(())
}

fn exact_ciphertext_modulus_addition_rns_bound(params: &BfvParameters) -> Result<u128, BfvError> {
    u128::from(params.ciphertext_modulus)
        .checked_mul(2)
        .and_then(|value| value.checked_sub(1))
        .ok_or_else(|| {
            BfvError::InvalidParameters(
                "BFV RNS exact ciphertext-modulus addition bound exceeds u128".to_owned(),
            )
        })
}

fn exact_ciphertext_modulus_negacyclic_product_abs_bound(
    params: &BfvParameters,
) -> Result<u128, BfvError> {
    let q_minus_one = u128::from(params.ciphertext_modulus - 1);
    u128::from(params.polynomial_degree)
        .checked_mul(q_minus_one)
        .and_then(|value| value.checked_mul(q_minus_one))
        .ok_or_else(|| {
            BfvError::InvalidParameters(
                "BFV RNS exact ciphertext-modulus negacyclic product bound exceeds u128".to_owned(),
            )
        })
}

fn exact_ciphertext_modulus_negacyclic_product_rns_bound(
    params: &BfvParameters,
) -> Result<u128, BfvError> {
    exact_ciphertext_modulus_negacyclic_product_abs_bound(params)?
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| {
            BfvError::InvalidParameters(
                "BFV RNS exact ciphertext-modulus negacyclic product bound exceeds u128".to_owned(),
            )
        })
}

fn reduce_u128_to_u64_mod(value: u128, modulus: u64) -> Result<u64, BfvError> {
    u64::try_from(value % u128::from(modulus)).map_err(|_| {
        BfvError::InvalidParameters(
            "BFV RNS reconstructed coefficient reduction exceeds u64".to_owned(),
        )
    })
}

fn reduce_centered_rns_value_to_u64_mod(
    value: u128,
    rns_product: u128,
    centered_abs_bound: u128,
    modulus: u64,
) -> Result<u64, BfvError> {
    if value <= centered_abs_bound {
        return reduce_u128_to_u64_mod(value, modulus);
    }
    let negative_magnitude = rns_product.checked_sub(value).ok_or_else(|| {
        BfvError::InvalidParameters(
            "BFV RNS centered reconstruction exceeds modulus-chain product".to_owned(),
        )
    })?;
    if negative_magnitude > centered_abs_bound {
        return Err(BfvError::InvalidParameters(
            "BFV RNS centered reconstruction exceeds exact negacyclic product bound".to_owned(),
        ));
    }
    let residue = reduce_u128_to_u64_mod(negative_magnitude, modulus)?;
    if residue == 0 {
        Ok(0)
    } else {
        Ok(modulus - residue)
    }
}

fn validate_rns_polynomial(
    params: &BfvParameters,
    chain: &BfvRnsModulusChain,
    polynomial: &BfvRnsPolynomial,
) -> Result<(), BfvError> {
    if polynomial.residues_by_limb.len() != chain.moduli.len() {
        return Err(BfvError::ShapeMismatch(format!(
            "RNS polynomial expected {} limbs, found {}",
            chain.moduli.len(),
            polynomial.residues_by_limb.len()
        )));
    }
    for (limb_index, (residues, &modulus)) in polynomial
        .residues_by_limb
        .iter()
        .zip(&chain.moduli)
        .enumerate()
    {
        if residues.len() != params.degree() {
            return Err(BfvError::ShapeMismatch(format!(
                "RNS polynomial limb {limb_index} expected polynomial_degree {}, found {}",
                params.polynomial_degree,
                residues.len()
            )));
        }
        if residues.iter().any(|&residue| residue >= modulus) {
            return Err(BfvError::ShapeMismatch(format!(
                "RNS polynomial limb {limb_index} contains a residue outside modulus {modulus}"
            )));
        }
    }
    Ok(())
}

fn validate_rns_polynomial_pair(
    params: &BfvParameters,
    chain: &BfvRnsModulusChain,
    lhs: &BfvRnsPolynomial,
    rhs: &BfvRnsPolynomial,
) -> Result<(), BfvError> {
    chain.validate_for_parameters(params)?;
    validate_rns_polynomial(params, chain, lhs)?;
    validate_rns_polynomial(params, chain, rhs)
}

fn multiply_rns_limb_negacyclic(
    params: &BfvParameters,
    lhs: &[u64],
    rhs: &[u64],
    modulus: u64,
) -> Vec<u64> {
    try_multiply_rns_limb_negacyclic_ntt(params, lhs, rhs, modulus)
        .unwrap_or_else(|| multiply_rns_limb_negacyclic_scalar(params, lhs, rhs, modulus))
}

fn multiply_rns_limb_negacyclic_scalar(
    params: &BfvParameters,
    lhs: &[u64],
    rhs: &[u64],
    modulus: u64,
) -> Vec<u64> {
    let degree = params.degree();
    let mut product = vec![0_u64; degree];
    for (lhs_index, &lhs_coefficient) in lhs.iter().enumerate() {
        for (rhs_index, &rhs_coefficient) in rhs.iter().enumerate() {
            let term = mul_mod_u64(lhs_coefficient, rhs_coefficient, modulus);
            let raw_index = lhs_index + rhs_index;
            if raw_index >= degree {
                let index = raw_index - degree;
                product[index] = sub_mod_u64(product[index], term, modulus);
            } else {
                product[raw_index] = add_mod_u64(product[raw_index], term, modulus);
            }
        }
    }
    product
}

fn try_multiply_rns_limb_negacyclic_ntt(
    params: &BfvParameters,
    lhs: &[u64],
    rhs: &[u64],
    modulus: u64,
) -> Option<Vec<u64>> {
    let degree = params.degree();
    if degree == 0 || lhs.len() != degree || rhs.len() != degree || !degree.is_power_of_two() {
        return None;
    }
    let root_order = u64::try_from(degree.checked_mul(2)?).ok()?;
    if modulus <= 2 || !(modulus - 1).is_multiple_of(root_order) {
        return None;
    }

    let psi = primitive_root_of_order(modulus, root_order)?;
    let omega = mul_mod_u64(psi, psi, modulus);
    let inv_psi = mod_inv_prime_u64(psi, modulus)?;
    let mut lhs_ntt = twist_rns_limb(lhs, psi, modulus);
    let mut rhs_ntt = twist_rns_limb(rhs, psi, modulus);
    ntt_in_place_mod(&mut lhs_ntt, omega, modulus, false)?;
    ntt_in_place_mod(&mut rhs_ntt, omega, modulus, false)?;
    for (left, right) in lhs_ntt.iter_mut().zip(&rhs_ntt) {
        *left = mul_mod_u64(*left, *right, modulus);
    }
    ntt_in_place_mod(&mut lhs_ntt, omega, modulus, true)?;
    untwist_rns_limb(&mut lhs_ntt, inv_psi, modulus);
    Some(lhs_ntt)
}

fn primitive_root_of_order(modulus: u64, order: u64) -> Option<u64> {
    if order == 0 || modulus <= 2 || !(modulus - 1).is_multiple_of(order) {
        return None;
    }
    let exponent = (modulus - 1) / order;
    let half_order = order / 2;
    for candidate in 2..modulus {
        let root = mod_pow_u64(candidate, exponent, modulus);
        if root != 1
            && mod_pow_u64(root, order, modulus) == 1
            && mod_pow_u64(root, half_order, modulus) == modulus - 1
        {
            return Some(root);
        }
    }
    None
}

fn twist_rns_limb(coefficients: &[u64], psi: u64, modulus: u64) -> Vec<u64> {
    let mut power = 1_u64;
    coefficients
        .iter()
        .map(|&coefficient| {
            let twisted = mul_mod_u64(coefficient, power, modulus);
            power = mul_mod_u64(power, psi, modulus);
            twisted
        })
        .collect()
}

fn untwist_rns_limb(coefficients: &mut [u64], inv_psi: u64, modulus: u64) {
    let mut power = 1_u64;
    for coefficient in coefficients {
        *coefficient = mul_mod_u64(*coefficient, power, modulus);
        power = mul_mod_u64(power, inv_psi, modulus);
    }
}

fn ntt_in_place_mod(values: &mut [u64], root: u64, modulus: u64, invert: bool) -> Option<()> {
    let len = values.len();
    if len == 0 || !len.is_power_of_two() || modulus <= 2 {
        return None;
    }
    bit_reverse_permute_rns(values);
    let root = if invert {
        mod_inv_prime_u64(root, modulus)?
    } else {
        root
    };

    let mut stage_len = 2_usize;
    while stage_len <= len {
        let step = mod_pow_u64(root, u64::try_from(len / stage_len).ok()?, modulus);
        for chunk in values.chunks_exact_mut(stage_len) {
            let (lo, hi) = chunk.split_at_mut(stage_len / 2);
            let mut twiddle = 1_u64;
            for (left, right) in lo.iter_mut().zip(hi.iter_mut()) {
                let product = mul_mod_u64(*right, twiddle, modulus);
                let left_value = *left;
                *left = add_mod_u64(left_value, product, modulus);
                *right = sub_mod_u64(left_value, product, modulus);
                twiddle = mul_mod_u64(twiddle, step, modulus);
            }
        }
        stage_len = stage_len.checked_mul(2)?;
    }

    if invert {
        let inv_len = mod_inv_prime_u64(u64::try_from(len).ok()?, modulus)?;
        for value in values {
            *value = mul_mod_u64(*value, inv_len, modulus);
        }
    }
    Some(())
}

fn bit_reverse_permute_rns(values: &mut [u64]) {
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

fn mod_inv_prime_u64(value: u64, modulus: u64) -> Option<u64> {
    if modulus <= 2 || value.is_multiple_of(modulus) {
        return None;
    }
    Some(mod_pow_u64(value, modulus - 2, modulus))
}

fn reconstruct_rns_coefficient(residues: &[u64], moduli: &[u64]) -> Result<u128, BfvError> {
    if residues.len() != moduli.len() {
        return Err(BfvError::ShapeMismatch(format!(
            "RNS coefficient expected {} residues, found {}",
            moduli.len(),
            residues.len()
        )));
    }
    let mut mixed = vec![0_u64; residues.len()];
    for (index, (&residue, &modulus)) in residues.iter().zip(moduli).enumerate() {
        let mut coefficient = residue;
        for (&prior, &prior_modulus) in mixed[..index].iter().zip(moduli.iter()) {
            coefficient = mul_mod_u64(
                sub_mod_u64(coefficient, prior, modulus),
                mod_pow_u64(prior_modulus % modulus, modulus - 2, modulus),
                modulus,
            );
        }
        mixed[index] = coefficient;
    }

    let mut value = 0_u128;
    let mut weight = 1_u128;
    for (index, &coefficient) in mixed.iter().enumerate() {
        let term = u128::from(coefficient).checked_mul(weight).ok_or_else(|| {
            BfvError::InvalidParameters("RNS coefficient reconstruction exceeds u128".to_owned())
        })?;
        value = value.checked_add(term).ok_or_else(|| {
            BfvError::InvalidParameters("RNS coefficient reconstruction exceeds u128".to_owned())
        })?;
        if index + 1 != mixed.len() {
            weight = weight
                .checked_mul(u128::from(moduli[index]))
                .ok_or_else(|| {
                    BfvError::InvalidParameters(
                        "RNS coefficient reconstruction exceeds u128".to_owned(),
                    )
                })?;
        }
    }
    Ok(value)
}

fn is_prime_u64(candidate: u64) -> bool {
    const SMALL_PRIMES: [u64; 12] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];
    const MILLER_RABIN_BASES: [u64; 7] = [2, 325, 9_375, 28_178, 450_775, 9_780_504, 1_795_265_022];

    if candidate < 2 {
        return false;
    }
    for prime in SMALL_PRIMES {
        if candidate == prime {
            return true;
        }
        if candidate.is_multiple_of(prime) {
            return false;
        }
    }

    let mut odd_factor = candidate - 1;
    let mut power_of_two = 0_u32;
    while odd_factor.is_multiple_of(2) {
        odd_factor /= 2;
        power_of_two = power_of_two.saturating_add(1);
    }

    'base: for base in MILLER_RABIN_BASES {
        let base = base % candidate;
        if base == 0 {
            continue;
        }
        let mut witness = mod_pow_u64(base, odd_factor, candidate);
        if witness == 1 || witness == candidate - 1 {
            continue;
        }
        for _ in 1..power_of_two {
            witness = mul_mod_u64(witness, witness, candidate);
            if witness == candidate - 1 {
                continue 'base;
            }
        }
        return false;
    }
    true
}

fn mod_pow_u64(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    let mut result = 1_u64;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = mul_mod_u64(result, base, modulus);
        }
        base = mul_mod_u64(base, base, modulus);
        exponent >>= 1;
    }
    result
}

fn gcd_u64(mut lhs: u64, mut rhs: u64) -> u64 {
    while rhs != 0 {
        let remainder = lhs % rhs;
        lhs = rhs;
        rhs = remainder;
    }
    lhs
}

fn derive_rng(domain: &[u8], seed: &[u8]) -> ChaCha20Rng {
    let material: [u8; Hash::LENGTH] = Hash::new_from_chunks(&[domain, seed]).into();
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
    Ok(Hash::new_from_chunks(&[IDENTIFIER_SLOT_ENCRYPT_DOMAIN, seed, &index.to_le_bytes()]).into())
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
    key_switch(params, &relinearization_key.entries, c0, c1, c2)
}

fn key_switch(
    params: &BfvParameters,
    entries: &[BfvRelinearizationKeyEntry],
    c0: &[u64],
    c1: &[u64],
    switching_component: &[u64],
) -> BfvCiphertext {
    let digits = decompose_poly(params, switching_component);
    let mut out0 = c0.to_vec();
    let mut out1 = c1.to_vec();
    for (digit_poly, entry) in digits.iter().zip(entries) {
        out0 = poly_add_mod(params, &out0, &poly_mul_mod(params, digit_poly, &entry.b));
        out1 = poly_add_mod(params, &out1, &poly_mul_mod(params, digit_poly, &entry.a));
    }
    BfvCiphertext { c0: out0, c1: out1 }
}

fn key_switch_from_transformed_secret(
    params: &BfvParameters,
    entries: &[BfvRelinearizationKeyEntry],
    c0: &[u64],
    transformed_c1: &[u64],
) -> BfvCiphertext {
    key_switch(params, entries, c0, &zero_poly(params), transformed_c1)
}

fn key_switch_entries_from_rng(
    params: &BfvParameters,
    source_secret: &[u64],
    target_secret: &[u64],
    rng: &mut ChaCha20Rng,
) -> Vec<BfvRelinearizationKeyEntry> {
    let digits = params.decomposition_digits();
    let base = params.decomposition_base();
    let mut scale = 1_u64;
    let mut entries = Vec::with_capacity(digits);
    for _ in 0..digits {
        let key_a = sample_uniform_poly(params, rng);
        let key_e = sample_error_poly(params, rng);
        let scaled_target = poly_scalar_mul_mod(params, target_secret, scale);
        let key_b = poly_add_mod(
            params,
            &poly_sub_mod(
                params,
                &poly_neg_mod(params, &poly_mul_mod(params, &key_a, source_secret)),
                &key_e,
            ),
            &scaled_target,
        );
        entries.push(BfvRelinearizationKeyEntry { b: key_b, a: key_a });
        scale = mul_mod_u64(scale, base, params.ciphertext_modulus);
    }
    entries
}

fn relinearize_rns_exact(
    params: &BfvParameters,
    rns_chain: &BfvRnsModulusChain,
    relinearization_key: &BfvRelinearizationKey,
    c0: &[u64],
    c1: &[u64],
    c2: &[u64],
) -> Result<BfvCiphertext, BfvError> {
    key_switch_rns_exact(params, rns_chain, &relinearization_key.entries, c0, c1, c2)
}

fn key_switch_from_transformed_secret_rns_exact(
    params: &BfvParameters,
    rns_chain: &BfvRnsModulusChain,
    entries: &[BfvRelinearizationKeyEntry],
    c0: &[u64],
    transformed_c1: &[u64],
) -> Result<BfvCiphertext, BfvError> {
    key_switch_rns_exact(
        params,
        rns_chain,
        entries,
        c0,
        &zero_poly(params),
        transformed_c1,
    )
}

fn key_switch_rns_exact(
    params: &BfvParameters,
    rns_chain: &BfvRnsModulusChain,
    entries: &[BfvRelinearizationKeyEntry],
    c0: &[u64],
    c1: &[u64],
    switching_component: &[u64],
) -> Result<BfvCiphertext, BfvError> {
    let digits = decompose_poly(params, switching_component);
    let mut out0 = c0.to_vec();
    let mut out1 = c1.to_vec();
    for (digit_poly, entry) in digits.iter().zip(entries) {
        let out0_contribution = rns_chain
            .multiply_ciphertext_modulus_polynomials_negacyclic_exact(
                params, digit_poly, &entry.b,
            )?;
        out0 = rns_chain.add_ciphertext_modulus_polynomials_exact(
            params,
            &out0,
            &out0_contribution,
        )?;
        let out1_contribution = rns_chain
            .multiply_ciphertext_modulus_polynomials_negacyclic_exact(
                params, digit_poly, &entry.a,
            )?;
        out1 = rns_chain.add_ciphertext_modulus_polynomials_exact(
            params,
            &out1,
            &out1_contribution,
        )?;
    }
    Ok(BfvCiphertext { c0: out0, c1: out1 })
}

fn apply_galois_automorphism_poly(
    params: &BfvParameters,
    poly: &[u64],
    automorphism_power: u32,
) -> Result<Polynomial, BfvError> {
    validate_poly(params, poly, "Galois automorphism polynomial")?;
    let power = validate_galois_automorphism_power(params, automorphism_power)?;
    let degree = params.degree();
    let cyclotomic_order = degree.checked_mul(2).ok_or_else(|| {
        BfvError::InvalidParameters(
            "BFV Galois automorphism order exceeds deterministic bounds".to_owned(),
        )
    })?;
    let mut output = zero_poly(params);
    for (index, &coefficient) in poly.iter().enumerate() {
        let exponent = index
            .checked_mul(power)
            .map(|value| value % cyclotomic_order)
            .ok_or_else(|| {
                BfvError::InvalidParameters(
                    "BFV Galois automorphism exponent exceeds deterministic bounds".to_owned(),
                )
            })?;
        if exponent >= degree {
            let target_index = exponent - degree;
            output[target_index] =
                sub_mod_u64(output[target_index], coefficient, params.ciphertext_modulus);
        } else {
            output[exponent] =
                add_mod_u64(output[exponent], coefficient, params.ciphertext_modulus);
        }
    }
    Ok(output)
}

fn packed_plaintext_evaluation_points(params: &BfvParameters) -> Result<Vec<u64>, BfvError> {
    let root = packed_plaintext_root(params)?;
    (0..params.degree())
        .map(|slot| {
            let exponent = slot
                .checked_mul(2)
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| {
                    BfvError::InvalidParameters(
                        "BFV packed plaintext slot exponent exceeds deterministic bounds"
                            .to_owned(),
                    )
                })?;
            let exponent = u64::try_from(exponent).map_err(|_| {
                BfvError::InvalidParameters(
                    "BFV packed plaintext slot exponent exceeds u64".to_owned(),
                )
            })?;
            Ok(mod_pow_u64(root, exponent, params.plaintext_modulus))
        })
        .collect()
}

fn packed_plaintext_root(params: &BfvParameters) -> Result<u64, BfvError> {
    params.validate()?;
    let degree = u64::from(params.polynomial_degree);
    let root_order = degree.checked_mul(2).ok_or_else(|| {
        BfvError::InvalidParameters(
            "BFV packed plaintext root order exceeds deterministic bounds".to_owned(),
        )
    })?;
    if !is_prime_u64(params.plaintext_modulus)
        || !(params.plaintext_modulus - 1).is_multiple_of(root_order)
    {
        return Err(BfvError::InvalidParameters(format!(
            "BFV packed plaintext slots require a prime plaintext modulus congruent to 1 mod {root_order}"
        )));
    }
    primitive_root_of_order(params.plaintext_modulus, root_order).ok_or_else(|| {
        BfvError::InvalidParameters("BFV packed plaintext primitive root is unavailable".to_owned())
    })
}

fn multiply_plaintext_linear_factor(coefficients: &[u64], root: u64, modulus: u64) -> Vec<u64> {
    let mut output = vec![0_u64; coefficients.len() + 1];
    for (index, &coefficient) in coefficients.iter().enumerate() {
        output[index] = add_mod_u64(
            output[index],
            mul_mod_u64(coefficient, sub_mod_u64(0, root, modulus), modulus),
            modulus,
        );
        output[index + 1] = add_mod_u64(output[index + 1], coefficient, modulus);
    }
    output
}

fn evaluate_plaintext_polynomial_mod(coefficients: &[u64], point: u64, modulus: u64) -> u64 {
    coefficients.iter().rev().fold(0_u64, |acc, &coefficient| {
        add_mod_u64(mul_mod_u64(acc, point, modulus), coefficient, modulus)
    })
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

fn rotation_steps_mod_slot_count(
    rotation_steps: u32,
    slot_count: usize,
) -> Result<usize, BfvError> {
    if slot_count == 0 {
        return Ok(0);
    }
    let slot_count = u64::try_from(slot_count).map_err(|_| {
        BfvError::InvalidParameters(
            "slot count exceeds deterministic BFV rotation bound".to_owned(),
        )
    })?;
    let normalized = u64::from(rotation_steps) % slot_count;
    usize::try_from(normalized).map_err(|_| {
        BfvError::InvalidParameters(
            "normalized BFV rotation step exceeds platform usize".to_owned(),
        )
    })
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

    fn rns_exact_params() -> BfvParameters {
        BfvParameters {
            polynomial_degree: 2,
            ciphertext_modulus: 45,
            plaintext_modulus: 5,
            decomposition_base_log: 4,
        }
    }

    fn rns_exact_chain() -> BfvRnsModulusChain {
        BfvRnsModulusChain {
            moduli: vec![73, 89, 97],
        }
    }

    struct EvaluationKeyAdversarialMaterial {
        params: BfvParameters,
        public_key: BfvPublicKey,
        relinearization_key: BfvRelinearizationKey,
        rotation_key: BfvRotationKey,
        galois_key: BfvGaloisKey,
        zero_refresh: BfvCiphertext,
    }

    fn evaluation_key_adversarial_material() -> EvaluationKeyAdversarialMaterial {
        let params = params();
        let (secret_key, public_key, relinearization_key) =
            keygen_from_seed(&params, b"bfv-eval-key-adversarial-keygen").expect("keygen");
        let rotation_key =
            rotation_key_from_seed(&params, &public_key, 1, b"bfv-eval-key-rotation")
                .expect("rotation key");
        let galois_key = galois_key_from_seed(&params, &secret_key, 3, b"bfv-eval-key-galois")
            .expect("Galois key");
        let zero_refresh =
            encrypt_from_seed(&params, &public_key, &[0], b"bfv-eval-key-zero-refresh")
                .expect("encrypt zero");

        EvaluationKeyAdversarialMaterial {
            params,
            public_key,
            relinearization_key,
            rotation_key,
            galois_key,
            zero_refresh,
        }
    }

    fn assert_evaluation_key_bundle_error_contains(
        params: &BfvParameters,
        bundle: &BfvEvaluationKeyBundle,
        expected: &str,
        context: &str,
    ) {
        let err = bundle.validate(params).expect_err(context);
        assert!(
            err.to_string().contains(expected),
            "expected `{expected}` in `{err}`"
        );
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
    fn rotation_key_large_steps_use_u64_modulo() {
        let params = params();
        let (secret_key, public_key, _) =
            keygen_from_seed(&params, b"bfv-rotation-large-keygen").expect("keygen");
        let slots = [10_u64, 20, 30, 40, 50, 60, 70]
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                encrypt_from_seed(
                    &params,
                    &public_key,
                    &[value],
                    format!("bfv-rotation-large-slot-{index}").as_bytes(),
                )
                .expect("encrypt slot")
            })
            .collect::<Vec<_>>();
        let rotation_key = rotation_key_from_seed(
            &params,
            &public_key,
            u32::MAX,
            b"bfv-rotation-large-zero-refresh",
        )
        .expect("rotation key");

        assert_eq!(
            rotation_steps_mod_slot_count(u32::MAX, slots.len()).expect("normalize rotation"),
            3
        );
        let rotated =
            rotate_ciphertext_slots_left(&params, &rotation_key, &slots).expect("rotate slots");

        let plaintexts = rotated
            .iter()
            .map(|slot| decrypt(&params, &secret_key, slot).expect("decrypt")[0])
            .collect::<Vec<_>>();
        assert_eq!(plaintexts, vec![40, 50, 60, 70, 10, 20, 30]);
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
    fn galois_key_switch_matches_plaintext_automorphism() {
        let params = params();
        let (secret_key, public_key, _) =
            keygen_from_seed(&params, b"bfv-galois-keygen").expect("keygen");
        let plaintext = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let ciphertext =
            encrypt_from_seed(&params, &public_key, &plaintext, b"bfv-galois-ciphertext")
                .expect("encrypt");
        let galois_key = galois_key_from_seed(&params, &secret_key, 3, b"bfv-galois-switch-key")
            .expect("Galois key");

        let transformed = apply_galois_automorphism_ciphertext(&params, &galois_key, &ciphertext)
            .expect("apply Galois automorphism");
        assert_ne!(transformed, ciphertext);

        let expected_encoded =
            apply_galois_automorphism_poly(&params, &encode_plaintext(&params, &plaintext), 3)
                .expect("automorph plaintext");
        let expected_plaintext = decode_plaintext(&params, &expected_encoded);
        let decrypted = decrypt(&params, &secret_key, &transformed).expect("decrypt transformed");
        assert_eq!(decrypted, expected_plaintext);
    }

    #[test]
    fn galois_key_switch_rns_exact_matches_scalar_baseline() {
        let params = rns_exact_params();
        let chain = rns_exact_chain();
        let (secret_key, public_key, _) =
            keygen_from_seed(&params, b"bfv-galois-rns-exact-keygen").expect("keygen");
        let plaintext = vec![1, 2];
        let ciphertext = encrypt_from_seed(
            &params,
            &public_key,
            &plaintext,
            b"bfv-galois-rns-exact-ciphertext",
        )
        .expect("encrypt");
        let galois_key =
            galois_key_from_seed(&params, &secret_key, 3, b"bfv-galois-rns-exact-switch-key")
                .expect("Galois key");

        let scalar = apply_galois_automorphism_ciphertext(&params, &galois_key, &ciphertext)
            .expect("scalar Galois switch");
        let rns = apply_galois_automorphism_ciphertext_rns_exact(
            &params,
            &chain,
            &galois_key,
            &ciphertext,
        )
        .expect("RNS exact Galois switch");
        assert_eq!(rns, scalar);

        let expected_encoded =
            apply_galois_automorphism_poly(&params, &encode_plaintext(&params, &plaintext), 3)
                .expect("automorph plaintext");
        let expected_plaintext = decode_plaintext(&params, &expected_encoded);
        assert_eq!(
            decrypt(&params, &secret_key, &rns).expect("decrypt RNS Galois switch"),
            expected_plaintext
        );
    }

    #[test]
    fn packed_plaintext_slots_roundtrip_registered_profile() {
        let params = ram_lfe_bfv_parameters_v1();
        let slots = (0..params.degree())
            .map(|index| u64::try_from(index * 7 + 3).expect("slot index fits u64") % 257)
            .collect::<Vec<_>>();

        let encoded = encode_packed_plaintext_slots(&params, &slots).expect("pack slots");
        assert_eq!(encoded.len(), params.degree());
        assert!(encoded.iter().all(|&coefficient| coefficient < 257));
        assert_eq!(
            decode_packed_plaintext_slots(&params, &encoded).expect("decode packed slots"),
            slots
        );
    }

    #[test]
    fn packed_plaintext_slots_require_batchable_parameters() {
        let params = params();
        let slots = vec![0; params.degree()];
        let err = encode_packed_plaintext_slots(&params, &slots)
            .expect_err("non-batchable plaintext modulus must fail");
        assert!(
            err.to_string().contains("packed plaintext slots require"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn galois_key_switch_matches_packed_slot_permutation() {
        let params = ram_lfe_bfv_parameters_v1();
        let (secret_key, public_key, _) =
            keygen_from_seed(&params, b"bfv-packed-galois-keygen").expect("keygen");
        let slots = (0..params.degree())
            .map(|index| u64::try_from(index + 1).expect("slot index fits u64"))
            .collect::<Vec<_>>();
        let packed_plaintext =
            encode_packed_plaintext_slots(&params, &slots).expect("encode packed slots");
        let ciphertext = encrypt_from_seed(
            &params,
            &public_key,
            &packed_plaintext,
            b"bfv-packed-galois-ciphertext",
        )
        .expect("encrypt packed plaintext");
        let galois_key =
            galois_key_from_seed(&params, &secret_key, 3, b"bfv-packed-galois-switch-key")
                .expect("Galois key");

        let transformed = apply_galois_automorphism_ciphertext(&params, &galois_key, &ciphertext)
            .expect("apply packed Galois switch");
        let transformed_plaintext =
            decrypt(&params, &secret_key, &transformed).expect("decrypt packed Galois output");
        let transformed_slots =
            decode_packed_plaintext_slots(&params, &transformed_plaintext).expect("decode slots");
        let permutation = packed_galois_slot_permutation(&params, 3).expect("slot permutation");
        let expected_slots = permutation
            .into_iter()
            .map(|input_index| slots[input_index])
            .collect::<Vec<_>>();
        assert_eq!(transformed_slots, expected_slots);
    }

    #[test]
    fn galois_keys_reject_noncanonical_powers_and_malformed_entries() {
        let params = params();
        let (secret_key, public_key, _) =
            keygen_from_seed(&params, b"bfv-galois-invalid-keygen").expect("keygen");
        for (power, message) in [(0, "1..16"), (2, "coprime"), (16, "1..16")] {
            let err = galois_key_from_seed(&params, &secret_key, power, b"bfv-galois-invalid")
                .expect_err("invalid Galois powers must be rejected");
            assert!(
                err.to_string().contains(message),
                "expected `{message}` in `{err}`"
            );
        }

        let ciphertext =
            encrypt_from_seed(&params, &public_key, &[1], b"bfv-galois-invalid-ciphertext")
                .expect("encrypt");
        let mut malformed = galois_key_from_seed(&params, &secret_key, 3, b"bfv-galois-malformed")
            .expect("Galois key");
        malformed.entries.pop();
        let err = apply_galois_automorphism_ciphertext(&params, &malformed, &ciphertext)
            .expect_err("malformed Galois key entries must be rejected");
        assert!(err.to_string().contains("Galois key expected"));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn evaluation_key_bundle_rejects_adversarial_rotation_and_bootstrap_metadata() {
        let material = evaluation_key_adversarial_material();
        let duplicate_rotation = BfvEvaluationKeyBundle {
            relinearization_key: material.relinearization_key.clone(),
            rotation_keys: vec![material.rotation_key.clone(), material.rotation_key.clone()],
            galois_keys: Vec::new(),
            bootstrap_key: None,
        };
        assert_evaluation_key_bundle_error_contains(
            &material.params,
            &duplicate_rotation,
            "duplicate rotation key",
            "duplicate rotation keys must be rejected",
        );

        let zero_step_rotation = BfvEvaluationKeyBundle {
            relinearization_key: material.relinearization_key.clone(),
            rotation_keys: vec![BfvRotationKey {
                rotation_steps: 0,
                zero_refresh: material.zero_refresh.clone(),
            }],
            galois_keys: Vec::new(),
            bootstrap_key: None,
        };
        assert_evaluation_key_bundle_error_contains(
            &material.params,
            &zero_step_rotation,
            "greater than zero",
            "zero-step rotation keys must be rejected",
        );

        let mut malformed_rotation_key = material.rotation_key;
        malformed_rotation_key.zero_refresh.c0.pop();
        let malformed_rotation = BfvEvaluationKeyBundle {
            relinearization_key: material.relinearization_key.clone(),
            rotation_keys: vec![malformed_rotation_key],
            galois_keys: Vec::new(),
            bootstrap_key: None,
        };
        assert_evaluation_key_bundle_error_contains(
            &material.params,
            &malformed_rotation,
            "ciphertext c0 length",
            "malformed rotation refresh ciphertext must be rejected",
        );

        let blank_bootstrap_key = BfvEvaluationKeyBundle {
            relinearization_key: material.relinearization_key.clone(),
            rotation_keys: Vec::new(),
            galois_keys: Vec::new(),
            bootstrap_key: Some(BfvBootstrapKey {
                key_id: "   ".to_owned(),
                max_refresh_rounds: BFV_BOOTSTRAP_KEY_DEFAULT_MAX_REFRESH_ROUNDS,
                zero_refresh: material.zero_refresh.clone(),
            }),
        };
        assert_evaluation_key_bundle_error_contains(
            &material.params,
            &blank_bootstrap_key,
            "bootstrap key id",
            "blank bootstrap key ids must be rejected",
        );

        let padded_bootstrap_key = BfvEvaluationKeyBundle {
            relinearization_key: material.relinearization_key.clone(),
            rotation_keys: Vec::new(),
            galois_keys: Vec::new(),
            bootstrap_key: Some(BfvBootstrapKey {
                key_id: " bootstrap-refresh-key".to_owned(),
                max_refresh_rounds: BFV_BOOTSTRAP_KEY_DEFAULT_MAX_REFRESH_ROUNDS,
                zero_refresh: material.zero_refresh.clone(),
            }),
        };
        let err = padded_bootstrap_key
            .digest(&material.params)
            .expect_err("padded bootstrap key ids must not receive digests");
        assert!(err.to_string().contains("canonical"));

        let control_bootstrap_key = BfvBootstrapKey {
            key_id: "bootstrap\nkey".to_owned(),
            max_refresh_rounds: BFV_BOOTSTRAP_KEY_DEFAULT_MAX_REFRESH_ROUNDS,
            zero_refresh: material.zero_refresh.clone(),
        };
        let err = bootstrap_ciphertext(
            &material.params,
            &control_bootstrap_key,
            &material.zero_refresh,
        )
        .expect_err("control bytes in bootstrap key ids must be rejected");
        assert!(err.to_string().contains("printable ASCII"));

        let err = bootstrap_key_from_seed(
            &material.params,
            &material.public_key,
            "a".repeat(BFV_BOOTSTRAP_KEY_ID_MAX_BYTES + 1),
            b"bfv-oversized-bootstrap-key-id",
        )
        .expect_err("oversized bootstrap key ids must be rejected");
        assert!(err.to_string().contains("maximum supported length"));

        let zero_round_key = BfvEvaluationKeyBundle {
            relinearization_key: material.relinearization_key.clone(),
            rotation_keys: Vec::new(),
            galois_keys: Vec::new(),
            bootstrap_key: Some(BfvBootstrapKey {
                key_id: "bootstrap-refresh-key".to_owned(),
                max_refresh_rounds: 0,
                zero_refresh: material.zero_refresh.clone(),
            }),
        };
        assert_evaluation_key_bundle_error_contains(
            &material.params,
            &zero_round_key,
            "max_refresh_rounds",
            "zero bootstrap refresh-round capacity must be rejected",
        );

        let err = bootstrap_key_with_max_refresh_rounds_from_seed(
            &material.params,
            &material.public_key,
            "bootstrap-refresh-key",
            BFV_BOOTSTRAP_KEY_MAX_REFRESH_ROUNDS + 1,
            b"bfv-oversized-bootstrap-refresh-rounds",
        )
        .expect_err("oversized bootstrap refresh-round capacity must be rejected");
        assert!(err.to_string().contains("supported limit"));
    }

    #[test]
    fn evaluation_key_bundle_rejects_adversarial_galois_and_bundle_metadata() {
        let material = evaluation_key_adversarial_material();
        let duplicate_galois = BfvEvaluationKeyBundle {
            relinearization_key: material.relinearization_key.clone(),
            rotation_keys: Vec::new(),
            galois_keys: vec![material.galois_key.clone(), material.galois_key.clone()],
            bootstrap_key: None,
        };
        assert_evaluation_key_bundle_error_contains(
            &material.params,
            &duplicate_galois,
            "duplicate Galois key",
            "duplicate Galois keys must be rejected",
        );

        let mut malformed_galois_key = material.galois_key.clone();
        malformed_galois_key.entries.pop();
        let malformed_galois = BfvEvaluationKeyBundle {
            relinearization_key: material.relinearization_key.clone(),
            rotation_keys: Vec::new(),
            galois_keys: vec![malformed_galois_key],
            bootstrap_key: None,
        };
        assert_evaluation_key_bundle_error_contains(
            &material.params,
            &malformed_galois,
            "Galois key expected",
            "malformed Galois key entries must be rejected",
        );

        let oversized_galois_bundle = BfvEvaluationKeyBundle {
            relinearization_key: material.relinearization_key.clone(),
            rotation_keys: Vec::new(),
            galois_keys: vec![material.galois_key; BFV_EVALUATION_KEY_MAX_GALOIS_KEYS + 1],
            bootstrap_key: None,
        };
        assert_evaluation_key_bundle_error_contains(
            &material.params,
            &oversized_galois_bundle,
            "Galois keys",
            "oversized Galois-key bundles must be rejected",
        );

        let oversized_rotation_bundle = BfvEvaluationKeyBundle {
            relinearization_key: material.relinearization_key,
            rotation_keys: (1..=u32::try_from(BFV_EVALUATION_KEY_MAX_ROTATION_KEYS + 1)
                .expect("rotation-key count fits into u32"))
                .map(|rotation_steps| BfvRotationKey {
                    rotation_steps,
                    zero_refresh: material.zero_refresh.clone(),
                })
                .collect(),
            galois_keys: Vec::new(),
            bootstrap_key: None,
        };
        assert_evaluation_key_bundle_error_contains(
            &material.params,
            &oversized_rotation_bundle,
            "rotation keys",
            "oversized rotation-key bundles must be rejected",
        );
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
    fn bfv_chunked_transcripts_match_legacy_contiguous_layout() {
        let params = ram_lfe_bfv_parameters_v1();

        let parameter_bytes = norito::to_bytes(&params).expect("encode BFV parameters");
        let legacy_parameter_digest =
            Hash::new([BFV_PARAMETER_DIGEST_DOMAIN, parameter_bytes.as_slice()].concat());
        assert_eq!(
            registered_bfv_parameter_digest(&params).expect("registered parameter digest"),
            legacy_parameter_digest
        );

        let (_, _, relinearization_key) =
            keygen_from_seed(&params, b"bfv-transcript-keygen").expect("keygen");
        let evaluation_keys = BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: Vec::new(),
            galois_keys: Vec::new(),
            bootstrap_key: None,
        };
        let evaluation_bytes =
            norito::to_bytes(&evaluation_keys).expect("encode BFV evaluation keys");
        let legacy_evaluation_digest = Hash::new(
            [
                BFV_EVALUATION_KEY_DIGEST_DOMAIN,
                evaluation_bytes.as_slice(),
            ]
            .concat(),
        );
        assert_eq!(
            evaluation_keys
                .digest(&params)
                .expect("evaluation-key digest"),
            legacy_evaluation_digest
        );

        let mut chunked_rng = derive_rng(b"bfv-rng-domain", b"bfv-rng-seed");
        let legacy_rng_seed: [u8; Hash::LENGTH] =
            Hash::new([b"bfv-rng-domain".as_slice(), b"bfv-rng-seed".as_slice()].concat()).into();
        let mut legacy_rng = <ChaCha20Rng as rand::SeedableRng>::from_seed(legacy_rng_seed);
        assert_eq!(
            sample_uniform_poly(&params, &mut chunked_rng),
            sample_uniform_poly(&params, &mut legacy_rng)
        );

        let (public_parameters, _, _) = derive_identifier_key_material_from_seed(
            &params,
            63,
            b"bfv-identifier-seed",
            b"phone#retail",
        )
        .expect("derive identifier key material");
        let legacy_identifier_seed: [u8; Hash::LENGTH] = Hash::new(
            [
                IDENTIFIER_KEYGEN_DOMAIN,
                b"phone#retail".as_slice(),
                b"bfv-identifier-seed".as_slice(),
            ]
            .concat(),
        )
        .into();
        let (_, legacy_public_key, _) =
            keygen_from_seed(&params, &legacy_identifier_seed).expect("legacy identifier keygen");
        assert_eq!(public_parameters.public_key, legacy_public_key);

        let index = 7_u64;
        let index_bytes = index.to_le_bytes();
        let legacy_slot_seed: [u8; Hash::LENGTH] = Hash::new(
            [
                IDENTIFIER_SLOT_ENCRYPT_DOMAIN,
                b"bfv-slot-seed".as_slice(),
                index_bytes.as_slice(),
            ]
            .concat(),
        )
        .into();
        assert_eq!(
            derive_identifier_slot_seed(
                b"bfv-slot-seed",
                usize::try_from(index).expect("test index fits usize"),
            )
            .expect("derive slot seed"),
            legacy_slot_seed
        );
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
    fn registered_rns_modulus_chain_covers_ram_lfe_profile() {
        let params = ram_lfe_bfv_parameters_v1();
        let chain =
            registered_bfv_rns_modulus_chain(&params).expect("registered RNS chain validates");

        assert_eq!(chain.moduli, RAM_LFE_BFV_RNS_MODULI_V1);
        assert_eq!(
            chain.product().expect("RNS chain product fits"),
            72_339_115_408_190_977
        );
        assert!(chain.product().expect("product") >= u128::from(params.ciphertext_modulus));
        chain
            .validate_for_parameters(&params)
            .expect("registered RNS chain covers BFV parameters");
        chain
            .validate_exact_lift_compatibility(&params)
            .expect("registered RNS chain fits the exact-lift fallback");

        let encoded_chain = norito::to_bytes(&chain).expect("encode RNS chain");
        let legacy_digest = Hash::new(
            [
                BFV_RNS_MODULUS_CHAIN_DIGEST_DOMAIN,
                encoded_chain.as_slice(),
            ]
            .concat(),
        );
        assert_eq!(
            chain
                .digest_for_parameters(&params)
                .expect("RNS chain digest"),
            legacy_digest
        );
        assert_eq!(
            registered_bfv_rns_modulus_chain_digest(&params).expect("registered RNS digest"),
            legacy_digest
        );
    }

    #[test]
    fn rns_modulus_chain_rejects_noncanonical_limb_shapes() {
        let params = ram_lfe_bfv_parameters_v1();
        let cases = [
            (BfvRnsModulusChain { moduli: vec![] }, "must not be empty"),
            (
                BfvRnsModulusChain {
                    moduli: vec![358_273; BFV_RNS_MODULUS_CHAIN_MAX_LIMBS + 1],
                },
                "at most",
            ),
            (
                BfvRnsModulusChain {
                    moduli: vec![358_273, 448_768, 449_921],
                },
                "must be odd",
            ),
            (
                BfvRnsModulusChain {
                    moduli: vec![358_273, 358_273, 449_921],
                },
                "strictly increasing",
            ),
            (
                BfvRnsModulusChain {
                    moduli: vec![385, 448_769, 449_921],
                },
                "must be prime",
            ),
            (
                BfvRnsModulusChain {
                    moduli: vec![263, 448_769, 449_921],
                },
                "1 mod",
            ),
        ];

        for (chain, expected_message) in cases {
            let err = chain
                .validate_for_parameters(&params)
                .expect_err("malformed RNS chains must be rejected");
            assert!(
                err.to_string().contains(expected_message),
                "expected `{expected_message}` in `{err}`"
            );
        }
    }

    #[test]
    fn rns_modulus_chain_rejects_insufficient_and_overflowing_products() {
        let params = ram_lfe_bfv_parameters_v1();
        let insufficient = BfvRnsModulusChain {
            moduli: vec![358_273, 448_769],
        };
        let err = insufficient
            .validate_for_parameters(&params)
            .expect_err("chain product must cover the ciphertext modulus");
        assert!(err.to_string().contains("does not cover"));

        let overflowing = BfvRnsModulusChain {
            moduli: vec![u64::MAX, u64::MAX, 2],
        };
        let err = overflowing
            .product()
            .expect_err("RNS product overflow must be reported");
        assert!(err.to_string().contains("exceeds u128"));
    }

    #[test]
    fn rns_modulus_chain_separates_rns_validity_from_exact_lift_bound() {
        let params = ram_lfe_bfv_parameters_v1();
        let wide_rns_chain = BfvRnsModulusChain {
            moduli: vec![4_292_018_177, 4_292_149_249],
        };

        wide_rns_chain
            .validate_for_parameters(&params)
            .expect("wide NTT-prime chain is structurally valid RNS");
        let err = wide_rns_chain
            .validate_exact_lift_compatibility(&params)
            .expect_err("current exact-lift fallback must reject oversized products");
        assert!(err.to_string().contains("exact-arithmetic"));
    }

    #[test]
    fn registered_rns_chain_rejects_naive_ciphertext_modulus_arithmetic_coverage() {
        let params = ram_lfe_bfv_parameters_v1();
        let chain = registered_bfv_rns_modulus_chain(&params).expect("registered RNS chain");
        let zero = vec![0; params.degree()];
        let (secret_key, _, relinearization_key) =
            keygen_from_seed(&params, b"bfv-registered-rns-exact-reject-keygen").expect("keygen");
        let galois_key =
            galois_key_from_seed(&params, &secret_key, 3, b"bfv-registered-rns-galois-reject")
                .expect("Galois key");
        let zero_ciphertext = zero_ciphertext(&params);

        let err = chain
            .validate_exact_ciphertext_modulus_addition_coverage(&params)
            .expect_err("registered chain is not wide enough for naive q-ring addition");
        assert!(err.to_string().contains("addition bound"));
        let err = chain
            .add_ciphertext_modulus_polynomials_exact(&params, &zero, &zero)
            .expect_err("registered chain must reject exact q-ring addition helper");
        assert!(err.to_string().contains("addition bound"));
        let err = add_ciphertexts_rns_exact(&params, &chain, &zero_ciphertext, &zero_ciphertext)
            .expect_err("registered chain must reject exact RNS ciphertext addition");
        assert!(err.to_string().contains("addition bound"));

        let err = chain
            .validate_exact_ciphertext_modulus_negacyclic_product_coverage(&params)
            .expect_err("registered chain is not wide enough for naive q-ring multiplication");
        assert!(err.to_string().contains("negacyclic product bound"));
        let err = chain
            .multiply_ciphertext_modulus_polynomials_negacyclic_exact(&params, &zero, &zero)
            .expect_err("registered chain must reject exact q-ring multiplication helper");
        assert!(err.to_string().contains("negacyclic product bound"));
        let err = multiply_ciphertexts_rns_exact(
            &params,
            &chain,
            &relinearization_key,
            &zero_ciphertext,
            &zero_ciphertext,
        )
        .expect_err("registered chain must reject exact RNS ciphertext multiplication");
        assert!(err.to_string().contains("negacyclic product bound"));
        let err = apply_galois_automorphism_ciphertext_rns_exact(
            &params,
            &chain,
            &galois_key,
            &zero_ciphertext,
        )
        .expect_err("registered chain must reject exact RNS Galois key switching");
        assert!(err.to_string().contains("negacyclic product bound"));
    }

    #[test]
    fn rns_chain_exact_ciphertext_modulus_arithmetic_guards_accept_wide_small_profile() {
        let params = rns_exact_params();
        let addition_only_chain = BfvRnsModulusChain { moduli: vec![73] };
        addition_only_chain
            .validate_for_parameters(&params)
            .expect("single-limb chain covers the small q");
        let err = addition_only_chain
            .validate_exact_ciphertext_modulus_addition_coverage(&params)
            .expect_err("single-limb chain remains narrower than the addition bound");
        assert!(err.to_string().contains("addition bound"));

        let wide_chain = BfvRnsModulusChain {
            moduli: vec![73, 89, 97],
        };
        wide_chain
            .validate_exact_ciphertext_modulus_addition_coverage(&params)
            .expect("wide chain covers exact q-ring addition");
        wide_chain
            .validate_exact_ciphertext_modulus_negacyclic_product_coverage(&params)
            .expect("wide chain covers exact q-ring negacyclic products");
    }

    #[test]
    fn rns_exact_ciphertext_modulus_polynomial_ops_match_scalar_q_ring() {
        let params = rns_exact_params();
        let chain = rns_exact_chain();
        chain
            .validate_exact_ciphertext_modulus_addition_coverage(&params)
            .expect("wide chain covers exact q-ring addition");
        chain
            .validate_exact_ciphertext_modulus_negacyclic_product_coverage(&params)
            .expect("wide chain covers exact q-ring multiplication");

        let lhs = vec![44, 7];
        let rhs = vec![12, 39];
        assert_eq!(
            chain
                .add_ciphertext_modulus_polynomials_exact(&params, &lhs, &rhs)
                .expect("exact RNS q-ring addition"),
            poly_add_mod(&params, &lhs, &rhs)
        );
        assert_eq!(
            chain
                .multiply_ciphertext_modulus_polynomials_negacyclic_exact(&params, &lhs, &rhs)
                .expect("exact RNS q-ring multiplication"),
            poly_mul_mod(&params, &lhs, &rhs)
        );

        let x = vec![0, 1];
        assert_eq!(
            chain
                .multiply_ciphertext_modulus_polynomials_negacyclic_exact(&params, &x, &x)
                .expect("x*x wraps as -1 in the negacyclic ring"),
            vec![params.ciphertext_modulus - 1, 0]
        );
    }

    #[test]
    fn rns_exact_ciphertext_evaluator_matches_scalar_baseline() {
        let params = rns_exact_params();
        let chain = rns_exact_chain();
        let (secret_key, public_key, relinearization_key) =
            keygen_from_seed(&params, b"bfv-rns-exact-ciphertext-keygen").expect("keygen");
        let lhs = encrypt_from_seed(
            &params,
            &public_key,
            &[params.plaintext_modulus - 1, 1],
            b"bfv-rns-exact-ciphertext-lhs",
        )
        .expect("encrypt lhs");
        let rhs = encrypt_from_seed(
            &params,
            &public_key,
            &[3, params.plaintext_modulus - 1],
            b"bfv-rns-exact-ciphertext-rhs",
        )
        .expect("encrypt rhs");

        let scalar_sum = add_ciphertexts(&params, &lhs, &rhs).expect("scalar add");
        let rns_sum =
            add_ciphertexts_rns_exact(&params, &chain, &lhs, &rhs).expect("RNS exact add");
        assert_eq!(rns_sum, scalar_sum);
        assert_eq!(
            &decrypt(&params, &secret_key, &rns_sum).expect("decrypt RNS sum")[..2],
            &[2, 0]
        );

        let scalar_product = multiply_ciphertexts(&params, &relinearization_key, &lhs, &rhs)
            .expect("scalar multiply");
        let rns_product =
            multiply_ciphertexts_rns_exact(&params, &chain, &relinearization_key, &lhs, &rhs)
                .expect("RNS exact multiply");
        assert_eq!(rns_product, scalar_product);
        assert_eq!(
            &decrypt(&params, &secret_key, &rns_product).expect("decrypt RNS product")[..2],
            &[3, 4]
        );
    }

    #[test]
    fn rns_polynomial_roundtrip_preserves_registered_ciphertext_coefficients() {
        let params = ram_lfe_bfv_parameters_v1();
        let chain = registered_bfv_rns_modulus_chain(&params).expect("registered chain");
        let mut coefficients = (0..params.degree())
            .map(|index| {
                let value = (u128::from(params.ciphertext_modulus - 1)
                    * u128::try_from(index + 17).expect("index fits"))
                    % u128::from(params.ciphertext_modulus);
                u64::try_from(value).expect("coefficient fits")
            })
            .collect::<Vec<_>>();
        coefficients[0] = 0;
        coefficients[1] = 1;
        coefficients[2] = RAM_LFE_BFV_PLAINTEXT_MODULUS;
        coefficients[3] = params.ciphertext_modulus - 1;

        let rns = chain
            .decompose_polynomial(&params, &coefficients)
            .expect("decompose polynomial into RNS");
        assert_eq!(rns.residues_by_limb.len(), chain.moduli.len());
        for (residues, &modulus) in rns.residues_by_limb.iter().zip(&chain.moduli) {
            assert_eq!(residues.len(), params.degree());
            assert!(residues.iter().all(|&residue| residue < modulus));
        }

        let reconstructed = chain
            .reconstruct_polynomial(&params, &rns)
            .expect("reconstruct RNS polynomial");
        assert_eq!(
            reconstructed,
            coefficients
                .iter()
                .map(|&coefficient| u128::from(coefficient))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn rns_polynomial_rejects_malformed_shapes_and_residues() {
        let params = ram_lfe_bfv_parameters_v1();
        let chain = registered_bfv_rns_modulus_chain(&params).expect("registered chain");
        let coefficients = vec![0; params.degree()];
        let rns = chain
            .decompose_polynomial(&params, &coefficients)
            .expect("decompose zero polynomial");

        let mut wrong_limb_count = rns.clone();
        wrong_limb_count.residues_by_limb.pop();
        let err = chain
            .reconstruct_polynomial(&params, &wrong_limb_count)
            .expect_err("RNS limb-count drift must be rejected");
        assert!(err.to_string().contains("expected 3 limbs"));

        let mut wrong_limb_len = rns.clone();
        wrong_limb_len.residues_by_limb[0].pop();
        let err = chain
            .reconstruct_polynomial(&params, &wrong_limb_len)
            .expect_err("RNS limb length drift must be rejected");
        assert!(err.to_string().contains("polynomial_degree"));

        let mut out_of_range = rns.clone();
        out_of_range.residues_by_limb[0][0] = chain.moduli[0];
        let err = chain
            .reconstruct_polynomial(&params, &out_of_range)
            .expect_err("RNS residues must be reduced modulo their limb");
        assert!(err.to_string().contains("outside modulus"));

        let mut outside_ciphertext_modulus = coefficients;
        outside_ciphertext_modulus[0] = params.ciphertext_modulus;
        let err = chain
            .decompose_polynomial(&params, &outside_ciphertext_modulus)
            .expect_err("BFV coefficients must stay inside ciphertext modulus");
        assert!(err.to_string().contains("outside ciphertext modulus"));
    }

    #[test]
    fn rns_polynomial_ring_operations_match_scalar_mod_chain_product() {
        let params = ram_lfe_bfv_parameters_v1();
        let chain = registered_bfv_rns_modulus_chain(&params).expect("registered chain");
        let product = chain.product().expect("chain product");
        let lhs_coefficients = (0..params.degree())
            .map(|index| {
                let value = (u128::try_from(index + 3).expect("index fits")
                    * u128::from(RAM_LFE_BFV_PLAINTEXT_MODULUS + 11))
                    % u128::from(params.ciphertext_modulus);
                u64::try_from(value).expect("lhs coefficient fits")
            })
            .collect::<Vec<_>>();
        let rhs_coefficients = (0..params.degree())
            .map(|index| {
                let value = (u128::try_from(index + 5).expect("index fits")
                    * u128::from(RAM_LFE_BFV_PLAINTEXT_MODULUS + 29))
                    % u128::from(params.ciphertext_modulus);
                u64::try_from(value).expect("rhs coefficient fits")
            })
            .collect::<Vec<_>>();
        let lhs = chain
            .decompose_polynomial(&params, &lhs_coefficients)
            .expect("decompose lhs");
        let rhs = chain
            .decompose_polynomial(&params, &rhs_coefficients)
            .expect("decompose rhs");

        let added = chain
            .add_rns_polynomials(&params, &lhs, &rhs)
            .expect("add RNS polynomials");
        let reconstructed_add = chain
            .reconstruct_polynomial(&params, &added)
            .expect("reconstruct RNS sum");
        let expected_add = lhs_coefficients
            .iter()
            .zip(&rhs_coefficients)
            .map(|(&left, &right)| (u128::from(left) + u128::from(right)) % product)
            .collect::<Vec<_>>();
        assert_eq!(reconstructed_add, expected_add);

        let multiplied = chain
            .multiply_rns_polynomials_negacyclic(&params, &lhs, &rhs)
            .expect("multiply RNS polynomials");
        let reconstructed_product = chain
            .reconstruct_polynomial(&params, &multiplied)
            .expect("reconstruct RNS product");
        let mut expected_product = vec![0_u128; params.degree()];
        for (lhs_index, &left) in lhs_coefficients.iter().enumerate() {
            for (rhs_index, &right) in rhs_coefficients.iter().enumerate() {
                let term = (u128::from(left) * u128::from(right)) % product;
                let raw_index = lhs_index + rhs_index;
                if raw_index >= params.degree() {
                    let index = raw_index - params.degree();
                    expected_product[index] = (expected_product[index] + product - term) % product;
                } else {
                    expected_product[raw_index] = (expected_product[raw_index] + term) % product;
                }
            }
        }
        assert_eq!(reconstructed_product, expected_product);
    }

    #[test]
    fn rns_negacyclic_ntt_limb_products_match_scalar_baseline() {
        let params = ram_lfe_bfv_parameters_v1();
        for &modulus in &RAM_LFE_BFV_RNS_MODULI_V1 {
            let lhs = (0..params.degree())
                .map(|index| {
                    let index = u64::try_from(index).expect("index fits");
                    ((index + 7) * (index + 11) * 17) % modulus
                })
                .collect::<Vec<_>>();
            let rhs = (0..params.degree())
                .map(|index| {
                    let index = u64::try_from(index).expect("index fits");
                    ((index + 5) * (index + 19) * 23) % modulus
                })
                .collect::<Vec<_>>();

            let ntt = try_multiply_rns_limb_negacyclic_ntt(&params, &lhs, &rhs, modulus)
                .expect("registered RNS limb must support negacyclic NTT");
            let scalar = multiply_rns_limb_negacyclic_scalar(&params, &lhs, &rhs, modulus);
            assert_eq!(ntt, scalar, "RNS limb {modulus} NTT product");
        }
    }

    #[test]
    fn rns_negacyclic_multiplication_falls_back_for_non_ntt_limb() {
        let params = BfvParameters {
            polynomial_degree: 8,
            ciphertext_modulus: 45,
            plaintext_modulus: 5,
            decomposition_base_log: 4,
        };
        let modulus = 19;
        let lhs = (0..params.degree())
            .map(|index| u64::try_from(index + 3).expect("index fits") % modulus)
            .collect::<Vec<_>>();
        let rhs = (0..params.degree())
            .map(|index| (u64::try_from(index + 5).expect("index fits") * 3) % modulus)
            .collect::<Vec<_>>();

        assert!(
            try_multiply_rns_limb_negacyclic_ntt(&params, &lhs, &rhs, modulus).is_none(),
            "non-NTT limb must not enter the NTT path"
        );
        assert_eq!(
            multiply_rns_limb_negacyclic(&params, &lhs, &rhs, modulus),
            multiply_rns_limb_negacyclic_scalar(&params, &lhs, &rhs, modulus)
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

        let err = registered_bfv_rns_modulus_chain(&params)
            .expect_err("unregistered BFV parameter sets must not receive RNS chains");
        assert!(err.to_string().contains("not registered"));

        let err = registered_bfv_rns_modulus_chain_digest(&params)
            .expect_err("unregistered BFV parameter sets must not receive RNS chain digests");
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
