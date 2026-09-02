//! Protocol-neutral transparent Goldilocks STARK primitives.
//!
//! This module contains only proof-system substrate: canonical Goldilocks base and
//! quartic-extension arithmetic, FFT/coset evaluation, zero-knowledge trace masking, framed
//! Fiat–Shamir, six-lane Poseidon Merkle commitments, binary FRI folding, grinding, and exact byte
//! readers/writers. Protocol relations and AIR constraints do not belong here. ZK-ACE, zk-X509,
//! private IVM, and PQ actions can therefore share one audited implementation without sharing or
//! weakening relations.
//!
//! The historical generic `crate::zk_stark` development envelope is not used: its query schedule
//! does not establish knowledge of the witness-bearing row. Callers of this substrate must commit
//! and query every masked witness column, bind composition quotients to those same openings, and
//! perform the complete FRI terminal-degree check.
pub(crate) use fastpq_prover::fastpq_isi_v1::GoldilocksDigest384V1;
#[cfg(any(test, feature = "privacy-release-evidence"))]
use fastpq_prover::fastpq_isi_v1::{
    GoldilocksDigest384LastFieldStreamErrorV1, GoldilocksDigest384LastFieldStreamV1,
};
use fastpq_prover::fastpq_isi_v1::{GoldilocksDigestDomainV1, hash_bytes_384_v1};
use iroha_data_model::privacy::{PRIVACY_EXACT12_CATALOG_COMMITMENT_WORDS_V1, PrivacyProtocolIdV1};
use rand::TryRngCore;
use rayon::prelude::*;
use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::BTreeSet;
use thiserror::Error;
use zeroize::Zeroize as _;
/// Goldilocks prime `2^64 - 2^32 + 1`.
pub(crate) const GOLDILOCKS_MODULUS_V1: u64 = 0xffff_ffff_0000_0001;
/// `2^64 - p = 2^32 - 1`, used for division-free canonical reduction.
const GOLDILOCKS_EPSILON_V1: u64 = 0xffff_ffff;
/// Canonical generator used for every compiled domain and coset.
pub(crate) const GOLDILOCKS_GENERATOR_V1: u64 = 7;
/// Two-adicity of the Goldilocks multiplicative group.
pub(crate) const GOLDILOCKS_TWO_ADICITY_V1: u32 = 32;
const TRANSCRIPT_FRAME_DOMAIN_V1: &[u8] = b"iroha:privacy:transparent-stark:frame:v1";
const TRANSCRIPT_INIT_DOMAIN_V1: &[u8] = b"iroha:privacy:transparent-stark:init:v1";
const TRANSCRIPT_ABSORB_DOMAIN_V1: &[u8] = b"iroha:privacy:transparent-stark:absorb:v1";
const TRANSCRIPT_CHALLENGE_DOMAIN_V1: &[u8] = b"iroha:privacy:transparent-stark:challenge:v1";
const TRANSCRIPT_FP4_CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha:privacy:transparent-stark:challenge:goldilocks-fp4:v1";
const QUERY_INDEX_DOMAIN_V1: &[u8] = b"iroha:privacy:transparent-stark:query-index:v1";
const GRINDING_DOMAIN_V1: &[u8] = b"iroha:privacy:transparent-stark:grinding:v1";
const MERKLE_NODE_PHASE_V1: &[u8] = b"binary-merkle-node";
/// Avoid Rayon dispatch overhead once a Merkle level becomes small.
const MERKLE_PARALLEL_PARENT_THRESHOLD_V1: usize = 256;
/// Avoid parallel dispatch for tiny development/test grinding targets.
const GRINDING_PARALLEL_MIN_BITS_V1: u8 = 12;
/// Search canonical nonce intervals in this fixed order while parallelizing within each interval.
const GRINDING_PARALLEL_CHUNK_SIZE_V1: u64 = 4_096;
const FRAME_PHASE_V1: &[u8] = b"framed-message";
/// Fixed rejection budget for canonical field and transcript sampling.
pub(crate) const MAX_FIELD_REJECTION_ATTEMPTS_V1: u64 = 16;
/// Fixed rejection budget for each unbiased query-index range sample.
const MAX_QUERY_INDEX_REJECTION_ATTEMPTS_V1: u64 = 256;
/// Degree of the compiled Goldilocks extension.
pub(crate) const GOLDILOCKS_FP4_DEGREE_V1: usize = 4;
/// Canonical encoded size of one quartic-extension value.
pub(crate) const GOLDILOCKS_FP4_WIRE_BYTES_V1: usize = GOLDILOCKS_FP4_DEGREE_V1 * 8;
const GOLDILOCKS_FP4_NONRESIDUE_V1: GoldilocksFieldV1 = GoldilocksFieldV1(GOLDILOCKS_GENERATOR_V1);

/// Exact catalog/protocol/profile context for every native-STARK digest.
///
/// The Exact12 catalog commitment is pinned internally, so a caller cannot
/// substitute a different catalog while retaining the same protocol and
/// profile labels. Protocol and profile identifiers are kept as distinct
/// fields and never concatenated into an ambiguous free-form domain string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TransparentStarkDigestContextV1 {
    protocol: PrivacyProtocolIdV1,
    profile: &'static [u8],
}
impl TransparentStarkDigestContextV1 {
    /// Construct a typed context for one final protocol/profile pair.
    pub(crate) const fn new(protocol: PrivacyProtocolIdV1, profile: &'static [u8]) -> Self {
        Self { protocol, profile }
    }
    pub(crate) fn validate(self) -> Result<(), TransparentStarkErrorV1> {
        if self.profile.is_empty() || u16::try_from(self.profile.len()).is_err() {
            return Err(TransparentStarkErrorV1::InvalidDigestDomain);
        }
        Ok(())
    }
}
/// Checked zero-knowledge masking geometry for the canonical DEEP-ALI flow.
///
/// `minimum_mask_coefficients` is the dimension `h` of the randomizer space
/// `Fp[X]_<h`; consequently its largest permitted monomial degree is `h - 1`.
/// Keeping those two quantities distinct prevents the consensus profile from
/// acquiring an off-by-one error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TransparentStarkZkMaskGeometryV1 {
    /// Reduced AIR degree (`d_air - 1` in Haböck--Al Kindi).
    pub(crate) reduced_air_degree: usize,
    /// Degree of the challenge field over the trace base field.
    pub(crate) extension_degree: usize,
    /// Number of extension-field DEEP samples.
    pub(crate) deep_query_count: usize,
    /// Number of base-domain FRI queries exposed for each witness oracle.
    pub(crate) fri_query_count: usize,
    /// Exact lower bound on the number of randomizer coefficients.
    pub(crate) minimum_mask_coefficients: usize,
    /// Largest degree of a minimum-size randomizer polynomial.
    pub(crate) minimum_mask_degree: usize,
}
/// Conservative classical-ROM work-normalized Fiat--Shamir certificate.
///
/// The caller must separately prove the supplied round-by-round soundness exponent for its concrete
/// FRI/DEEP construction. This helper checks the protocol-neutral BCS accounting
///
/// `epsilon_FS / Q <= epsilon_RBR + 3 * (Q + 1/Q) / 2^kappa`
///
/// using an exact power-of-two split of the target error budget. It deliberately does not use
/// floating-point arithmetic and makes no qROM or post-quantum claim for the Fiat--Shamir layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TransparentStarkWorkSecurityV1 {
    /// Claimed work-normalized security level.
    pub(crate) target_bits: u16,
    /// Proven exponent in `epsilon_RBR <= 2^-round_by_round_bits`.
    pub(crate) round_by_round_bits: u16,
    /// Random-oracle digest size.
    pub(crate) random_oracle_bits: u16,
    /// Bound `Q <= 2^max_random_oracle_query_log2`.
    pub(crate) max_random_oracle_query_log2: u16,
}
/// Canonical Goldilocks field element.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GoldilocksFieldV1(pub(crate) u64);
impl GoldilocksFieldV1 {
    /// Additive identity.
    pub(crate) const ZERO: Self = Self(0);
    /// Multiplicative identity.
    pub(crate) const ONE: Self = Self(1);
    /// Decode one canonical residue.
    pub(crate) fn canonical(value: u64) -> Option<Self> {
        (value < GOLDILOCKS_MODULUS_V1).then_some(Self(value))
    }
    /// Reduce a 128-bit value modulo the Goldilocks prime.
    ///
    /// For `p = 2^64 - 2^32 + 1`, `2^64 = 2^32 - 1 (mod p)`.
    /// Splitting the high limb once more at bit 32 yields
    ///
    /// `lo + hi * 2^64 = lo - hi_hi + hi_lo * (2^32 - 1) (mod p)`.
    ///
    /// The two overflow corrections replace an implicit `2^64` with the
    /// congruent `2^32 - 1`; the final value needs at most one subtraction.
    /// This is exact for every `u128`, not only products of canonical residues.
    pub(crate) fn reduce(value: u128) -> Self {
        let low = value as u64;
        let high = (value >> 64) as u64;
        let high_high = high >> 32;
        let high_low = high & GOLDILOCKS_EPSILON_V1;
        let (reduced_low, borrowed) = low.overflowing_sub(high_high);
        let reduced_low = if borrowed {
            reduced_low.wrapping_sub(GOLDILOCKS_EPSILON_V1)
        } else {
            reduced_low
        };
        let folded_high = high_low * GOLDILOCKS_EPSILON_V1;
        let (reduced, carried) = reduced_low.overflowing_add(folded_high);
        let reduced = if carried {
            reduced.wrapping_add(GOLDILOCKS_EPSILON_V1)
        } else {
            reduced
        };
        Self(Self::canonicalize_once_v1(reduced))
    }
    /// Canonical residue as a `u64`.
    pub(crate) const fn value(self) -> u64 {
        self.0
    }
    /// Reliably overwrite this field element before releasing secret-bearing storage.
    pub(crate) fn zeroize_v1(&mut self) {
        self.0.zeroize();
    }
    /// Field addition.
    pub(crate) fn add(self, rhs: Self) -> Self {
        debug_assert!(self.0 < GOLDILOCKS_MODULUS_V1);
        debug_assert!(rhs.0 < GOLDILOCKS_MODULUS_V1);
        let (sum, carried) = self.0.overflowing_add(rhs.0);
        let sum = if carried {
            // The wrapped sum omitted one `2^64`, which is epsilon modulo p.
            sum.wrapping_add(GOLDILOCKS_EPSILON_V1)
        } else {
            sum
        };
        Self(Self::canonicalize_once_v1(sum))
    }
    /// Field subtraction.
    pub(crate) fn sub(self, rhs: Self) -> Self {
        if self.0 >= rhs.0 {
            Self(self.0 - rhs.0)
        } else {
            Self(GOLDILOCKS_MODULUS_V1 - (rhs.0 - self.0))
        }
    }
    /// Field multiplication.
    pub(crate) fn mul(self, rhs: Self) -> Self {
        debug_assert!(self.0 < GOLDILOCKS_MODULUS_V1);
        debug_assert!(rhs.0 < GOLDILOCKS_MODULUS_V1);
        Self::reduce(u128::from(self.0) * u128::from(rhs.0))
    }
    fn canonicalize_once_v1(value: u64) -> u64 {
        if value >= GOLDILOCKS_MODULUS_V1 {
            value - GOLDILOCKS_MODULUS_V1
        } else {
            value
        }
    }
    /// Exponentiation by repeated squaring.
    pub(crate) fn pow(mut self, mut exponent: u128) -> Self {
        let mut result = Self::ONE;
        while exponent != 0 {
            if exponent & 1 == 1 {
                result = result.mul(self);
            }
            self = self.mul(self);
            exponent >>= 1;
        }
        result
    }
    /// Multiplicative inverse, absent for zero.
    pub(crate) fn inv(self) -> Option<Self> {
        (self != Self::ZERO && self.0 < GOLDILOCKS_MODULUS_V1)
            .then(|| self.pow(u128::from(GOLDILOCKS_MODULUS_V1 - 2)))
    }
}
/// Quartic extension of Goldilocks defined by `w^4 = 7`.
///
/// Coefficients are in ascending power order: `c[0] + c[1] w + c[2] w^2 + c[3] w^3`. Seven is a
/// quadratic non-residue in Goldilocks and the prime is one modulo four. A monic factorization of
/// `X^4 - 7` into two quadratics would therefore imply that either `7` or `-7` is a square; both
/// are impossible because `-1` is a square. Thus this quotient is a field, rather than merely a
/// ring.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GoldilocksFp4V1 {
    coefficients: [GoldilocksFieldV1; GOLDILOCKS_FP4_DEGREE_V1],
}
impl GoldilocksFp4V1 {
    /// Additive identity.
    pub(crate) const ZERO: Self = Self {
        coefficients: [GoldilocksFieldV1::ZERO; GOLDILOCKS_FP4_DEGREE_V1],
    };
    /// Multiplicative identity.
    pub(crate) const ONE: Self = Self {
        coefficients: [
            GoldilocksFieldV1::ONE,
            GoldilocksFieldV1::ZERO,
            GoldilocksFieldV1::ZERO,
            GoldilocksFieldV1::ZERO,
        ],
    };
    /// Decode four canonical residues in ascending power order.
    pub(crate) fn canonical(values: [u64; 4]) -> Option<Self> {
        Self::from_coefficients([
            GoldilocksFieldV1::canonical(values[0])?,
            GoldilocksFieldV1::canonical(values[1])?,
            GoldilocksFieldV1::canonical(values[2])?,
            GoldilocksFieldV1::canonical(values[3])?,
        ])
    }
    /// Decode the canonical fixed-width big-endian wire encoding.
    pub(crate) fn canonical_be_bytes(bytes: [u8; 32]) -> Option<Self> {
        let mut values = [0_u64; GOLDILOCKS_FP4_DEGREE_V1];
        for (index, chunk) in bytes.chunks_exact(8).enumerate() {
            values[index] = u64::from_be_bytes(
                chunk
                    .try_into()
                    .expect("a 32-byte array has four exact 8-byte chunks"),
            );
        }
        Self::canonical(values)
    }
    /// Construct from four already-decoded coefficients.
    pub(crate) fn from_coefficients(coefficients: [GoldilocksFieldV1; 4]) -> Option<Self> {
        coefficients
            .iter()
            .all(|coefficient| coefficient.0 < GOLDILOCKS_MODULUS_V1)
            .then_some(Self { coefficients })
    }
    /// Embed one base-field element.
    pub(crate) fn from_base(value: GoldilocksFieldV1) -> Self {
        debug_assert!(value.0 < GOLDILOCKS_MODULUS_V1);
        Self {
            coefficients: [
                value,
                GoldilocksFieldV1::ZERO,
                GoldilocksFieldV1::ZERO,
                GoldilocksFieldV1::ZERO,
            ],
        }
    }
    /// Return the four canonical coefficients in ascending power order.
    pub(crate) const fn coefficients(self) -> [GoldilocksFieldV1; 4] {
        self.coefficients
    }
    /// Return the canonical fixed-width big-endian wire encoding.
    pub(crate) fn to_be_bytes(self) -> [u8; 32] {
        let mut bytes = [0_u8; GOLDILOCKS_FP4_WIRE_BYTES_V1];
        for (index, coefficient) in self.coefficients.iter().enumerate() {
            let start = index * 8;
            bytes[start..start + 8].copy_from_slice(&coefficient.0.to_be_bytes());
        }
        bytes
    }
    /// Whether every coefficient is a canonical base-field residue.
    pub(crate) fn is_canonical(self) -> bool {
        self.coefficients
            .iter()
            .all(|coefficient| coefficient.0 < GOLDILOCKS_MODULUS_V1)
    }
    /// Reliably overwrite every coefficient before releasing secret-bearing storage.
    pub(crate) fn zeroize_v1(&mut self) {
        for coefficient in &mut self.coefficients {
            coefficient.zeroize_v1();
        }
    }
    /// Field addition.
    pub(crate) fn add(self, rhs: Self) -> Self {
        let mut coefficients = [GoldilocksFieldV1::ZERO; GOLDILOCKS_FP4_DEGREE_V1];
        for (target, (left, right)) in coefficients.iter_mut().zip(
            self.coefficients
                .iter()
                .copied()
                .zip(rhs.coefficients.iter().copied()),
        ) {
            *target = left.add(right);
        }
        Self { coefficients }
    }
    /// Field subtraction.
    pub(crate) fn sub(self, rhs: Self) -> Self {
        let mut coefficients = [GoldilocksFieldV1::ZERO; GOLDILOCKS_FP4_DEGREE_V1];
        for (target, (left, right)) in coefficients.iter_mut().zip(
            self.coefficients
                .iter()
                .copied()
                .zip(rhs.coefficients.iter().copied()),
        ) {
            *target = left.sub(right);
        }
        Self { coefficients }
    }
    /// Additive inverse.
    #[cfg_attr(not(any(test, feature = "zk-stark")), allow(dead_code))]
    pub(crate) fn neg(self) -> Self {
        Self::ZERO.sub(self)
    }
    /// Field multiplication modulo `w^4 - 7`.
    pub(crate) fn mul(self, rhs: Self) -> Self {
        let mut product = [GoldilocksFieldV1::ZERO; 7];
        for (left_degree, left) in self.coefficients.iter().copied().enumerate() {
            for (right_degree, right) in rhs.coefficients.iter().copied().enumerate() {
                let degree = left_degree + right_degree;
                product[degree] = product[degree].add(left.mul(right));
            }
        }
        for degree in 4..=6 {
            product[degree - 4] =
                product[degree - 4].add(product[degree].mul(GOLDILOCKS_FP4_NONRESIDUE_V1));
        }
        Self {
            coefficients: [product[0], product[1], product[2], product[3]],
        }
    }
    /// Multiply every coefficient by one base-field element.
    pub(crate) fn mul_base(self, rhs: GoldilocksFieldV1) -> Self {
        debug_assert!(rhs.0 < GOLDILOCKS_MODULUS_V1);
        let mut coefficients = self.coefficients;
        for coefficient in &mut coefficients {
            *coefficient = coefficient.mul(rhs);
        }
        Self { coefficients }
    }
    /// Exponentiation by a `u128` exponent.
    pub(crate) fn pow(mut self, mut exponent: u128) -> Self {
        let mut result = Self::ONE;
        while exponent != 0 {
            if exponent & 1 == 1 {
                result = result.mul(self);
            }
            self = self.mul(self);
            exponent >>= 1;
        }
        result
    }
    /// Multiplicative inverse, absent for zero.
    ///
    /// Write the element as `A + B w` over the quadratic subfield `Fp[u] / (u^2 - 7)`, where `u =
    /// w^2`. Then `(A + B w)^-1 = (A - B w) / (A^2 - B^2 u)`.
    pub(crate) fn inv(self) -> Option<Self> {
        if self == Self::ZERO || !self.is_canonical() {
            return None;
        }
        let even = [self.coefficients[0], self.coefficients[2]];
        let odd = [self.coefficients[1], self.coefficients[3]];
        let even_squared = goldilocks_fp2_mul_v1(even, even);
        let odd_squared = goldilocks_fp2_mul_v1(odd, odd);
        let odd_squared_times_u = [
            odd_squared[1].mul(GOLDILOCKS_FP4_NONRESIDUE_V1),
            odd_squared[0],
        ];
        let denominator = [
            even_squared[0].sub(odd_squared_times_u[0]),
            even_squared[1].sub(odd_squared_times_u[1]),
        ];
        let denominator_inverse = goldilocks_fp2_inv_v1(denominator)?;
        let inverse_even = goldilocks_fp2_mul_v1(even, denominator_inverse);
        let inverse_odd = goldilocks_fp2_mul_v1(odd, denominator_inverse);
        Some(Self {
            coefficients: [
                inverse_even[0],
                GoldilocksFieldV1::ZERO.sub(inverse_odd[0]),
                inverse_even[1],
                GoldilocksFieldV1::ZERO.sub(inverse_odd[1]),
            ],
        })
    }
}
fn goldilocks_fp2_mul_v1(
    left: [GoldilocksFieldV1; 2],
    right: [GoldilocksFieldV1; 2],
) -> [GoldilocksFieldV1; 2] {
    [
        left[0]
            .mul(right[0])
            .add(left[1].mul(right[1]).mul(GOLDILOCKS_FP4_NONRESIDUE_V1)),
        left[0].mul(right[1]).add(left[1].mul(right[0])),
    ]
}
fn goldilocks_fp2_inv_v1(value: [GoldilocksFieldV1; 2]) -> Option<[GoldilocksFieldV1; 2]> {
    let norm = value[0]
        .mul(value[0])
        .sub(value[1].mul(value[1]).mul(GOLDILOCKS_FP4_NONRESIDUE_V1));
    let norm_inverse = norm.inv()?;
    Some([
        value[0].mul(norm_inverse),
        GoldilocksFieldV1::ZERO.sub(value[1]).mul(norm_inverse),
    ])
}
/// Failure in protocol-neutral transparent-proof machinery.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum TransparentStarkErrorV1 {
    /// A catalog-bound protocol/profile/role/phase digest domain is invalid.
    #[error("transparent STARK digest domain is invalid")]
    InvalidDigestDomain,
    /// A power-of-two domain shape or degree bound is invalid.
    #[error("transparent STARK domain shape is invalid")]
    InvalidDomain,
    /// The requested FFT domain exceeds Goldilocks two-adicity.
    #[error("transparent STARK domain exceeds Goldilocks two-adicity")]
    DomainTooLarge,
    /// A required inverse does not exist.
    #[error("transparent STARK attempted to invert zero")]
    DivisionByZero,
    /// An encoded field value is not canonical.
    #[error("transparent STARK field encoding is non-canonical")]
    NonCanonicalField,
    /// A Merkle tree or opening has an invalid shape.
    #[error("transparent STARK Merkle shape is invalid")]
    InvalidMerkleShape,
    /// Canonical transcript framing overflowed.
    #[error("transparent STARK transcript frame length overflow")]
    FrameLengthOverflow,
    /// Fiat–Shamir sampling exhausted its fixed rejection bound.
    #[error("transparent STARK Fiat-Shamir rejection bound exhausted")]
    ChallengeSamplingExhausted,
    /// Unique query-index derivation exhausted its fixed work bound.
    #[error("transparent STARK query-index derivation exhausted")]
    QuerySamplingExhausted,
    /// The operating-system or injected random source failed.
    #[error("transparent STARK masking randomness is unavailable")]
    RandomnessUnavailable,
    /// A complete terminal polynomial exceeds the required degree.
    #[error("transparent STARK FRI terminal degree is too high")]
    FriDegree,
    /// A proof byte stream is truncated or has a trailing suffix.
    #[error("transparent STARK proof bytes are malformed")]
    MalformedProof,
    /// Exact bounded allocation failed.
    #[error("transparent STARK bounded allocation failed")]
    AllocationFailure,
    /// The configured grinding nonce does not meet its bit target.
    #[error("transparent STARK grinding nonce is invalid")]
    InvalidGrinding,
}
/// Derive the exact minimum Protocol-3 masking geometry.
///
/// This is Equation (3) of Haböck--Al Kindi, ePrint 2024/1037:
///
/// `2 * d * (e * n_DEEP + n_FRI) + n_FRI <= h`.
///
/// Here `h` counts coefficients because the sampled randomizer belongs to
/// `Fp[X]_<h`. A DEEP-free local subproof may pass zero for
/// `deep_query_count`; FRI still requires at least one query.
pub(crate) fn transparent_stark_zk_mask_geometry_v1(
    reduced_air_degree: usize,
    extension_degree: usize,
    deep_query_count: usize,
    fri_query_count: usize,
) -> Result<TransparentStarkZkMaskGeometryV1, TransparentStarkErrorV1> {
    if reduced_air_degree == 0 || extension_degree == 0 || fri_query_count == 0 {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    let extension_deep_queries = extension_degree
        .checked_mul(deep_query_count)
        .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
    let revealed_queries = extension_deep_queries
        .checked_add(fri_query_count)
        .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
    let minimum_mask_coefficients = 2_usize
        .checked_mul(reduced_air_degree)
        .and_then(|factor| factor.checked_mul(revealed_queries))
        .and_then(|implicit| implicit.checked_add(fri_query_count))
        .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
    let minimum_mask_degree = minimum_mask_coefficients
        .checked_sub(1)
        .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
    Ok(TransparentStarkZkMaskGeometryV1 {
        reduced_air_degree,
        extension_degree,
        deep_query_count,
        fri_query_count,
        minimum_mask_coefficients,
        minimum_mask_degree,
    })
}
/// Check a classical-ROM work-normalized BCS/Fiat--Shamir claim without rounding.
///
/// Half of the target error budget is assigned to round-by-round soundness and
/// half to the random-oracle term. For `Q <= 2^q`,
/// `3 * (Q + 1/Q) / 2^kappa < 2^(q + 3 - kappa)`, so the checked conditions
/// are `rbr_bits >= lambda + 1` and `q <= kappa - lambda - 4`.
pub(crate) fn checked_transparent_stark_work_security_v1(
    target_bits: u16,
    round_by_round_bits: u16,
    random_oracle_bits: u16,
    max_random_oracle_query_log2: u16,
) -> Result<TransparentStarkWorkSecurityV1, TransparentStarkErrorV1> {
    if target_bits == 0 {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    let minimum_round_by_round_bits = target_bits
        .checked_add(1)
        .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
    let maximum_query_log2 = random_oracle_bits
        .checked_sub(target_bits)
        .and_then(|remaining| remaining.checked_sub(4))
        .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
    if round_by_round_bits < minimum_round_by_round_bits
        || max_random_oracle_query_log2 > maximum_query_log2
    {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    Ok(TransparentStarkWorkSecurityV1 {
        target_bits,
        round_by_round_bits,
        random_oracle_bits,
        max_random_oracle_query_log2,
    })
}
/// Compute the primitive root for an exact power-of-two order.
pub(crate) fn goldilocks_primitive_root_v1(
    log_size: u8,
) -> Result<GoldilocksFieldV1, TransparentStarkErrorV1> {
    if u32::from(log_size) > GOLDILOCKS_TWO_ADICITY_V1 {
        return Err(TransparentStarkErrorV1::DomainTooLarge);
    }
    let order = 1_u128 << log_size;
    let root = GoldilocksFieldV1(GOLDILOCKS_GENERATOR_V1)
        .pow((u128::from(GOLDILOCKS_MODULUS_V1) - 1) / order);
    if root.pow(order) != GoldilocksFieldV1::ONE
        || (order > 1 && root.pow(order / 2) == GoldilocksFieldV1::ONE)
    {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    Ok(root)
}
/// In-place radix-two FFT.
pub(crate) fn goldilocks_fft_v1(
    values: &mut [GoldilocksFieldV1],
    root: GoldilocksFieldV1,
) -> Result<(), TransparentStarkErrorV1> {
    let size = values.len();
    if size == 0
        || !size.is_power_of_two()
        || root.0 >= GOLDILOCKS_MODULUS_V1
        || root.pow(size as u128) != GoldilocksFieldV1::ONE
        || (size > 1 && root.pow((size / 2) as u128) == GoldilocksFieldV1::ONE)
    {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    if values.iter().any(|value| value.0 >= GOLDILOCKS_MODULUS_V1) {
        return Err(TransparentStarkErrorV1::NonCanonicalField);
    }
    let mut reversed = 0_usize;
    for index in 1..size {
        let mut bit = size >> 1;
        while reversed & bit != 0 {
            reversed ^= bit;
            bit >>= 1;
        }
        reversed ^= bit;
        if index < reversed {
            values.swap(index, reversed);
        }
    }
    let mut width = 2_usize;
    while width <= size {
        let step = root.pow((size / width) as u128);
        for chunk in values.chunks_exact_mut(width) {
            let mut twiddle = GoldilocksFieldV1::ONE;
            let (left, right) = chunk.split_at_mut(width / 2);
            for (even, odd) in left.iter_mut().zip(right.iter_mut()) {
                let scaled_odd = (*odd).mul(twiddle);
                let original_even = *even;
                *even = original_even.add(scaled_odd);
                *odd = original_even.sub(scaled_odd);
                twiddle = twiddle.mul(step);
            }
        }
        width <<= 1;
    }
    Ok(())
}
/// In-place inverse radix-two FFT.
pub(crate) fn goldilocks_ifft_v1(
    values: &mut [GoldilocksFieldV1],
    root: GoldilocksFieldV1,
) -> Result<(), TransparentStarkErrorV1> {
    if root.0 >= GOLDILOCKS_MODULUS_V1 {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    goldilocks_fft_v1(
        values,
        root.inv().ok_or(TransparentStarkErrorV1::DivisionByZero)?,
    )?;
    let inverse_size = GoldilocksFieldV1::reduce(values.len() as u128)
        .inv()
        .ok_or(TransparentStarkErrorV1::DivisionByZero)?;
    for value in values {
        *value = value.mul(inverse_size);
    }
    Ok(())
}
/// Evaluate coefficients over one shifted radix-two domain.
pub(crate) fn goldilocks_evaluate_coset_v1(
    coefficients: &[GoldilocksFieldV1],
    size: usize,
    root: GoldilocksFieldV1,
    shift: GoldilocksFieldV1,
) -> Result<Vec<GoldilocksFieldV1>, TransparentStarkErrorV1> {
    if coefficients.len() > size
        || size == 0
        || !size.is_power_of_two()
        || shift == GoldilocksFieldV1::ZERO
        || shift.0 >= GOLDILOCKS_MODULUS_V1
    {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    if coefficients
        .iter()
        .any(|coefficient| coefficient.0 >= GOLDILOCKS_MODULUS_V1)
    {
        return Err(TransparentStarkErrorV1::NonCanonicalField);
    }
    let mut evaluations = vec![GoldilocksFieldV1::ZERO; size];
    let mut shift_power = GoldilocksFieldV1::ONE;
    for (target, coefficient) in evaluations.iter_mut().zip(coefficients.iter().copied()) {
        *target = coefficient.mul(shift_power);
        shift_power = shift_power.mul(shift);
    }
    goldilocks_fft_v1(&mut evaluations, root)?;
    Ok(evaluations)
}
/// In-place radix-two FFT over the quartic Goldilocks extension.
///
/// The evaluation domain remains in the base field, so roots and twiddles are
/// embedded rather than sampled from the extension.
pub(crate) fn goldilocks_fp4_fft_v1(
    values: &mut [GoldilocksFp4V1],
    root: GoldilocksFieldV1,
) -> Result<(), TransparentStarkErrorV1> {
    let size = values.len();
    if size == 0
        || !size.is_power_of_two()
        || root.0 >= GOLDILOCKS_MODULUS_V1
        || root.pow(size as u128) != GoldilocksFieldV1::ONE
        || (size > 1 && root.pow((size / 2) as u128) == GoldilocksFieldV1::ONE)
    {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    if values.iter().any(|value| !value.is_canonical()) {
        return Err(TransparentStarkErrorV1::NonCanonicalField);
    }
    let mut reversed = 0_usize;
    for index in 1..size {
        let mut bit = size >> 1;
        while reversed & bit != 0 {
            reversed ^= bit;
            bit >>= 1;
        }
        reversed ^= bit;
        if index < reversed {
            values.swap(index, reversed);
        }
    }
    let mut width = 2_usize;
    while width <= size {
        let step = root.pow((size / width) as u128);
        for chunk in values.chunks_exact_mut(width) {
            let mut twiddle = GoldilocksFieldV1::ONE;
            let (left, right) = chunk.split_at_mut(width / 2);
            for (even, odd) in left.iter_mut().zip(right.iter_mut()) {
                let scaled_odd = (*odd).mul_base(twiddle);
                let original_even = *even;
                *even = original_even.add(scaled_odd);
                *odd = original_even.sub(scaled_odd);
                twiddle = twiddle.mul(step);
            }
        }
        width <<= 1;
    }
    Ok(())
}
/// In-place inverse radix-two FFT over the quartic Goldilocks extension.
pub(crate) fn goldilocks_fp4_ifft_v1(
    values: &mut [GoldilocksFp4V1],
    root: GoldilocksFieldV1,
) -> Result<(), TransparentStarkErrorV1> {
    if root.0 >= GOLDILOCKS_MODULUS_V1 {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    goldilocks_fp4_fft_v1(
        values,
        root.inv().ok_or(TransparentStarkErrorV1::DivisionByZero)?,
    )?;
    let inverse_size = GoldilocksFieldV1::reduce(values.len() as u128)
        .inv()
        .ok_or(TransparentStarkErrorV1::DivisionByZero)?;
    for value in values {
        *value = value.mul_base(inverse_size);
    }
    Ok(())
}
/// Evaluate quartic-extension coefficients over one shifted base-field domain.
pub(crate) fn goldilocks_fp4_evaluate_coset_v1(
    coefficients: &[GoldilocksFp4V1],
    size: usize,
    root: GoldilocksFieldV1,
    shift: GoldilocksFieldV1,
) -> Result<Vec<GoldilocksFp4V1>, TransparentStarkErrorV1> {
    if coefficients.len() > size
        || size == 0
        || !size.is_power_of_two()
        || shift == GoldilocksFieldV1::ZERO
        || shift.0 >= GOLDILOCKS_MODULUS_V1
    {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    if coefficients.iter().any(|value| !value.is_canonical()) {
        return Err(TransparentStarkErrorV1::NonCanonicalField);
    }
    let mut evaluations = vec![GoldilocksFp4V1::ZERO; size];
    let mut shift_power = GoldilocksFieldV1::ONE;
    for (target, coefficient) in evaluations.iter_mut().zip(coefficients.iter().copied()) {
        *target = coefficient.mul_base(shift_power);
        shift_power = shift_power.mul(shift);
    }
    goldilocks_fp4_fft_v1(&mut evaluations, root)?;
    Ok(evaluations)
}
/// Batch-invert a non-empty collection using one field inversion.
pub(crate) fn goldilocks_batch_invert_v1(
    values: &mut [GoldilocksFieldV1],
) -> Result<(), TransparentStarkErrorV1> {
    if values.is_empty() {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    if values.iter().any(|value| value.0 >= GOLDILOCKS_MODULUS_V1) {
        return Err(TransparentStarkErrorV1::NonCanonicalField);
    }
    let mut prefixes = Vec::new();
    prefixes
        .try_reserve_exact(values.len())
        .map_err(|_| TransparentStarkErrorV1::AllocationFailure)?;
    let mut product = GoldilocksFieldV1::ONE;
    for value in values.iter().copied() {
        if value == GoldilocksFieldV1::ZERO {
            return Err(TransparentStarkErrorV1::DivisionByZero);
        }
        prefixes.push(product);
        product = product.mul(value);
    }
    let mut inverse = product
        .inv()
        .ok_or(TransparentStarkErrorV1::DivisionByZero)?;
    for index in (0..values.len()).rev() {
        let value = values[index];
        values[index] = inverse.mul(prefixes[index]);
        inverse = inverse.mul(value);
    }
    Ok(())
}
/// Draw one unbiased canonical Goldilocks field element.
pub(crate) fn random_goldilocks_v1<R: TryRngCore>(
    rng: &mut R,
) -> Result<GoldilocksFieldV1, TransparentStarkErrorV1> {
    for _ in 0..MAX_FIELD_REJECTION_ATTEMPTS_V1 {
        let mut bytes = [0_u8; 8];
        rng.try_fill_bytes(&mut bytes)
            .map_err(|_| TransparentStarkErrorV1::RandomnessUnavailable)?;
        if let Some(value) = GoldilocksFieldV1::canonical(u64::from_le_bytes(bytes)) {
            return Ok(value);
        }
    }
    Err(TransparentStarkErrorV1::RandomnessUnavailable)
}
/// Draw one uniform quartic-extension element, including zero.
pub(crate) fn random_goldilocks_fp4_v1<R: TryRngCore>(
    rng: &mut R,
) -> Result<GoldilocksFp4V1, TransparentStarkErrorV1> {
    GoldilocksFp4V1::from_coefficients([
        random_goldilocks_v1(rng)?,
        random_goldilocks_v1(rng)?,
        random_goldilocks_v1(rng)?,
        random_goldilocks_v1(rng)?,
    ])
    .ok_or(TransparentStarkErrorV1::NonCanonicalField)
}
/// Draw one uniform nonzero quartic-extension element.
#[cfg(test)]
pub(crate) fn random_nonzero_goldilocks_fp4_v1<R: TryRngCore>(
    rng: &mut R,
) -> Result<GoldilocksFp4V1, TransparentStarkErrorV1> {
    for _ in 0..MAX_FIELD_REJECTION_ATTEMPTS_V1 {
        let value = random_goldilocks_fp4_v1(rng)?;
        if value != GoldilocksFp4V1::ZERO {
            return Ok(value);
        }
    }
    Err(TransparentStarkErrorV1::RandomnessUnavailable)
}
/// Interpolate one native trace column and apply an exact replayable mask.
///
/// For a native domain of size `n`, the returned polynomial is `T(X) + r(X) * (X^n - 1)`. Its
/// ascending coefficient vector has exactly `n + r.len()` entries, including canonical trailing
/// zero coefficients. Keeping this operation separate from coset evaluation lets bounded provers
/// retain the much smaller polynomial while replaying commitments on more than one verifier-derived
/// evaluation domain.
pub(crate) fn masked_trace_coefficients_with_mask_v1(
    base_column: &[GoldilocksFieldV1],
    base_log_size: u8,
    mask: &[GoldilocksFieldV1],
) -> Result<Vec<GoldilocksFieldV1>, TransparentStarkErrorV1> {
    if mask.is_empty() {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    if mask
        .iter()
        .any(|coefficient| coefficient.0 >= GOLDILOCKS_MODULUS_V1)
    {
        return Err(TransparentStarkErrorV1::NonCanonicalField);
    }
    let base_size = 1_usize
        .checked_shl(u32::from(base_log_size))
        .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
    let coefficient_count = base_size
        .checked_add(mask.len())
        .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
    if base_column.len() != base_size {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    let base_root = goldilocks_primitive_root_v1(base_log_size)?;
    let mut coefficients = ZeroizingGoldilocksValuesV1(Vec::new());
    coefficients
        .try_reserve_exact(coefficient_count)
        .map_err(|_| TransparentStarkErrorV1::AllocationFailure)?;
    coefficients.extend_from_slice(base_column);
    goldilocks_ifft_v1(&mut coefficients, base_root)?;
    coefficients.resize(coefficient_count, GoldilocksFieldV1::ZERO);
    for (degree, random) in mask.iter().copied().enumerate() {
        coefficients[degree] = coefficients[degree].sub(random);
        coefficients[base_size + degree] = coefficients[base_size + degree].add(random);
    }
    Ok(coefficients.into_inner())
}
/// Evaluate retained masked trace coefficients on one canonical generator coset.
///
/// The evaluation domain may be smaller than the eventual commitment domain,
/// but it must contain every coefficient and remain disjoint from both the
/// native trace subgroup and its own evaluation subgroup.
pub(crate) fn masked_trace_coefficients_on_coset_v1(
    coefficients: &[GoldilocksFieldV1],
    base_log_size: u8,
    evaluation_log_size: u8,
) -> Result<Vec<GoldilocksFieldV1>, TransparentStarkErrorV1> {
    let base_size = 1_usize
        .checked_shl(u32::from(base_log_size))
        .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
    let evaluation_size = 1_usize
        .checked_shl(u32::from(evaluation_log_size))
        .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
    if coefficients.is_empty()
        || coefficients.len() > evaluation_size
        || evaluation_size <= base_size
    {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    let evaluation_root = goldilocks_primitive_root_v1(evaluation_log_size)?;
    let shift = GoldilocksFieldV1(GOLDILOCKS_GENERATOR_V1);
    if shift.pow(base_size as u128) == GoldilocksFieldV1::ONE
        || shift.pow(evaluation_size as u128) == GoldilocksFieldV1::ONE
    {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    goldilocks_evaluate_coset_v1(coefficients, evaluation_size, evaluation_root, shift)
}
/// Interpolate and mask one trace column before evaluating its LDE.
///
/// The mask is `r(X) * (X^n - 1)`, so every base-domain trace value is unchanged while all queried
/// coset values are randomized. `mask_degree` is inclusive.
pub(crate) fn masked_trace_lde_column_with_mask_v1(
    base_column: &[GoldilocksFieldV1],
    base_log_size: u8,
    lde_log_size: u8,
    mask: &[GoldilocksFieldV1],
) -> Result<Vec<GoldilocksFieldV1>, TransparentStarkErrorV1> {
    let coefficients = ZeroizingGoldilocksValuesV1(masked_trace_coefficients_with_mask_v1(
        base_column,
        base_log_size,
        mask,
    )?);
    // These coefficients interpolate the native witness. The guard wipes them on success,
    // ordinary failure, and unwind before a Rayon worker can return allocator storage to a
    // long-lived pool.
    masked_trace_coefficients_on_coset_v1(&coefficients, base_log_size, lde_log_size)
}
struct ZeroizingGoldilocksValuesV1(Vec<GoldilocksFieldV1>);
impl ZeroizingGoldilocksValuesV1 {
    fn into_inner(mut self) -> Vec<GoldilocksFieldV1> {
        core::mem::take(&mut self.0)
    }
}
impl core::ops::Deref for ZeroizingGoldilocksValuesV1 {
    type Target = Vec<GoldilocksFieldV1>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl core::ops::DerefMut for ZeroizingGoldilocksValuesV1 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Drop for ZeroizingGoldilocksValuesV1 {
    fn drop(&mut self) {
        for coefficient in &mut self.0 {
            coefficient.zeroize_v1();
        }
    }
}
/// Replayable zero-knowledge mask for one streamed trace column.
pub(crate) struct ReplayableTraceMaskV1 {
    coefficients: Vec<GoldilocksFieldV1>,
}
impl ReplayableTraceMaskV1 {
    /// Exact coefficients in ascending degree order.
    pub(crate) fn coefficients(&self) -> &[GoldilocksFieldV1] {
        &self.coefficients
    }
}
impl Drop for ReplayableTraceMaskV1 {
    fn drop(&mut self) {
        for coefficient in &mut self.coefficients {
            coefficient.zeroize_v1();
        }
    }
}
/// Sample and retain the exact mask coefficients for one replayable column.
///
/// Streaming provers keep these few coefficients until post-query openings are reconstructed,
/// instead of retaining the entire LDE column. Callers should drop them as soon as proof
/// construction completes; [`Drop`] overwrites the backing allocation before release.
pub(crate) fn sample_trace_mask_v1<R: TryRngCore>(
    mask_degree: usize,
    rng: &mut R,
) -> Result<ReplayableTraceMaskV1, TransparentStarkErrorV1> {
    let mask_len = mask_degree
        .checked_add(1)
        .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
    let mut mask = Vec::new();
    mask.try_reserve_exact(mask_len)
        .map_err(|_| TransparentStarkErrorV1::AllocationFailure)?;
    for _ in 0..mask_len {
        mask.push(random_goldilocks_v1(rng)?);
    }
    Ok(ReplayableTraceMaskV1 { coefficients: mask })
}
/// Interpolate, sample a fresh mask, and evaluate one trace column's LDE.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn masked_trace_lde_column_v1<R: TryRngCore>(
    base_column: &[GoldilocksFieldV1],
    base_log_size: u8,
    lde_log_size: u8,
    mask_degree: usize,
    rng: &mut R,
) -> Result<Vec<GoldilocksFieldV1>, TransparentStarkErrorV1> {
    let mask = sample_trace_mask_v1(mask_degree, rng)?;
    masked_trace_lde_column_with_mask_v1(
        base_column,
        base_log_size,
        lde_log_size,
        mask.coefficients(),
    )
}
fn exact12_catalog_commitment_bytes_v1() -> [u8; 48] {
    GoldilocksDigest384V1::new(PRIVACY_EXACT12_CATALOG_COMMITMENT_WORDS_V1)
        .expect("the pinned Exact12 catalog commitment is canonical")
        .to_le_bytes()
}

/// Hash an unambiguous domain-and-field frame with SHA-256.
pub(crate) fn sha256_frame_v1(
    domain: &[u8],
    fields: &[&[u8]],
) -> Result<[u8; 32], TransparentStarkErrorV1> {
    let domain_len =
        u16::try_from(domain.len()).map_err(|_| TransparentStarkErrorV1::FrameLengthOverflow)?;
    let field_count =
        u16::try_from(fields.len()).map_err(|_| TransparentStarkErrorV1::FrameLengthOverflow)?;
    let mut hash = Sha256::new();
    hash.update(TRANSCRIPT_FRAME_DOMAIN_V1);
    hash.update(domain_len.to_be_bytes());
    hash.update(domain);
    hash.update(field_count.to_be_bytes());
    for field in fields {
        let length =
            u64::try_from(field.len()).map_err(|_| TransparentStarkErrorV1::FrameLengthOverflow)?;
        hash.update(length.to_be_bytes());
        hash.update(field);
    }
    Ok(hash.finalize().into())
}

/// Hash one fully typed native-STARK frame with the canonical six-lane digest.
pub(crate) fn goldilocks_digest384_frame_v1(
    context: TransparentStarkDigestContextV1,
    role: &[u8],
    phase: &[u8],
    level: u64,
    index: u64,
    counter: u64,
    fields: &[&[u8]],
) -> Result<GoldilocksDigest384V1, TransparentStarkErrorV1> {
    context.validate()?;
    if role.is_empty()
        || phase.is_empty()
        || u16::try_from(role.len()).is_err()
        || u16::try_from(phase.len()).is_err()
    {
        return Err(TransparentStarkErrorV1::InvalidDigestDomain);
    }
    let catalog = exact12_catalog_commitment_bytes_v1();
    hash_bytes_384_v1(
        GoldilocksDigestDomainV1 {
            catalog: &catalog,
            protocol: context.protocol.canonical_label().as_bytes(),
            profile: context.profile,
            role,
            phase,
            level,
            index,
            counter,
        },
        fields,
    )
    .ok_or(TransparentStarkErrorV1::FrameLengthOverflow)
}

/// Start a bounded digest stream whose final framed field is supplied incrementally.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn goldilocks_digest384_last_field_stream_v1(
    context: TransparentStarkDigestContextV1,
    role: &[u8],
    phase: &[u8],
    level: u64,
    index: u64,
    counter: u64,
    prefix_fields: &[&[u8]],
    final_field_len: usize,
) -> Result<GoldilocksDigest384LastFieldStreamV1, TransparentStarkErrorV1> {
    context.validate()?;
    if role.is_empty()
        || phase.is_empty()
        || u16::try_from(role.len()).is_err()
        || u16::try_from(phase.len()).is_err()
    {
        return Err(TransparentStarkErrorV1::InvalidDigestDomain);
    }
    let catalog = exact12_catalog_commitment_bytes_v1();
    GoldilocksDigest384LastFieldStreamV1::new(
        GoldilocksDigestDomainV1 {
            catalog: &catalog,
            protocol: context.protocol.canonical_label().as_bytes(),
            profile: context.profile,
            role,
            phase,
            level,
            index,
            counter,
        },
        prefix_fields,
        final_field_len,
    )
    .map_err(|_| TransparentStarkErrorV1::FrameLengthOverflow)
}

/// Map a canonical digest-stream failure without exposing an alternate hash path.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) const fn map_digest_stream_error_v1(
    error: GoldilocksDigest384LastFieldStreamErrorV1,
) -> TransparentStarkErrorV1 {
    match error {
        GoldilocksDigest384LastFieldStreamErrorV1::FramingLimitExceeded
        | GoldilocksDigest384LastFieldStreamErrorV1::InputOverrun { .. }
        | GoldilocksDigest384LastFieldStreamErrorV1::InputUnderrun { .. } => {
            TransparentStarkErrorV1::FrameLengthOverflow
        }
    }
}

/// Domain-separated binary six-lane Poseidon Merkle tree.
#[derive(Clone, Debug)]
pub(crate) struct GoldilocksMerkleTreeV1 {
    levels: Vec<Vec<GoldilocksDigest384V1>>,
}
impl GoldilocksMerkleTreeV1 {
    /// Commit a non-empty power-of-two leaf vector.
    pub(crate) fn from_leaves(
        leaves: Vec<GoldilocksDigest384V1>,
        context: TransparentStarkDigestContextV1,
        node_role: &'static [u8],
    ) -> Result<Self, TransparentStarkErrorV1> {
        context.validate()?;
        if leaves.is_empty()
            || !leaves.len().is_power_of_two()
            || node_role.is_empty()
            || u16::try_from(node_role.len()).is_err()
        {
            return Err(TransparentStarkErrorV1::InvalidMerkleShape);
        }
        let mut levels = Vec::new();
        levels
            .try_reserve_exact(
                usize::try_from(leaves.len().ilog2())
                    .ok()
                    .and_then(|depth| depth.checked_add(1))
                    .ok_or(TransparentStarkErrorV1::InvalidMerkleShape)?,
            )
            .map_err(|_| TransparentStarkErrorV1::AllocationFailure)?;
        levels.push(leaves);
        while levels.last().map_or(0, Vec::len) > 1 {
            let parent_level = u64::try_from(levels.len())
                .map_err(|_| TransparentStarkErrorV1::InvalidMerkleShape)?;
            let previous = levels
                .last()
                .ok_or(TransparentStarkErrorV1::InvalidMerkleShape)?;
            let parent_count = previous.len() / 2;
            let mut next = Vec::new();
            next.try_reserve_exact(parent_count)
                .map_err(|_| TransparentStarkErrorV1::AllocationFailure)?;
            next.resize(parent_count, GoldilocksDigest384V1::default());
            // Every node is domain-separated by its canonical level and index. An indexed
            // parallel write therefore changes only scheduling: the resulting vector and root
            // are byte-identical for every Rayon pool width and hardware topology.
            let hash_parent =
                |index: usize| -> Result<GoldilocksDigest384V1, TransparentStarkErrorV1> {
                    let child_index = index
                        .checked_mul(2)
                        .ok_or(TransparentStarkErrorV1::InvalidMerkleShape)?;
                    goldilocks_merkle_node_v1(
                        context,
                        node_role,
                        parent_level,
                        u64::try_from(index)
                            .map_err(|_| TransparentStarkErrorV1::InvalidMerkleShape)?,
                        previous[child_index],
                        previous[child_index + 1],
                    )
                };
            if parent_count >= MERKLE_PARALLEL_PARENT_THRESHOLD_V1 {
                next.par_iter_mut().enumerate().try_for_each(
                    |(index, parent)| -> Result<(), TransparentStarkErrorV1> {
                        *parent = hash_parent(index)?;
                        Ok(())
                    },
                )?;
            } else {
                for (index, parent) in next.iter_mut().enumerate() {
                    *parent = hash_parent(index)?;
                }
            }
            levels.push(next);
        }
        Ok(Self { levels })
    }
    /// Root digest.
    pub(crate) fn root(&self) -> GoldilocksDigest384V1 {
        self.levels[self.levels.len() - 1][0]
    }
    /// Leaf-to-root sibling path.
    pub(crate) fn path(
        &self,
        mut index: usize,
    ) -> Result<Vec<GoldilocksDigest384V1>, TransparentStarkErrorV1> {
        if index >= self.levels[0].len() {
            return Err(TransparentStarkErrorV1::InvalidMerkleShape);
        }
        let mut path = Vec::new();
        path.try_reserve_exact(self.levels.len() - 1)
            .map_err(|_| TransparentStarkErrorV1::AllocationFailure)?;
        for level in &self.levels[..self.levels.len() - 1] {
            path.push(level[index ^ 1]);
            index >>= 1;
        }
        Ok(path)
    }
}
/// Hash one binary Merkle node with an engine-fixed role domain.
pub(crate) fn goldilocks_merkle_node_v1(
    context: TransparentStarkDigestContextV1,
    node_role: &[u8],
    level: u64,
    index: u64,
    left: GoldilocksDigest384V1,
    right: GoldilocksDigest384V1,
) -> Result<GoldilocksDigest384V1, TransparentStarkErrorV1> {
    goldilocks_digest384_frame_v1(
        context,
        node_role,
        MERKLE_NODE_PHASE_V1,
        level,
        index,
        0,
        &[&left.to_le_bytes(), &right.to_le_bytes()],
    )
}
/// Verify one exact binary Merkle path.
#[cfg(test)]
pub(crate) fn verify_goldilocks_merkle_path_v1(
    context: TransparentStarkDigestContextV1,
    node_role: &[u8],
    root: GoldilocksDigest384V1,
    mut leaf: GoldilocksDigest384V1,
    mut index: usize,
    path: &[GoldilocksDigest384V1],
    expected_depth: usize,
) -> Result<(), TransparentStarkErrorV1> {
    context.validate()?;
    if node_role.is_empty()
        || u16::try_from(node_role.len()).is_err()
        || path.len() != expected_depth
    {
        return Err(TransparentStarkErrorV1::InvalidMerkleShape);
    }
    for (path_level, sibling) in path.iter().copied().enumerate() {
        let parent_index = index >> 1;
        leaf = if index & 1 == 0 {
            goldilocks_merkle_node_v1(
                context,
                node_role,
                u64::try_from(path_level + 1)
                    .map_err(|_| TransparentStarkErrorV1::InvalidMerkleShape)?,
                u64::try_from(parent_index)
                    .map_err(|_| TransparentStarkErrorV1::InvalidMerkleShape)?,
                leaf,
                sibling,
            )?
        } else {
            goldilocks_merkle_node_v1(
                context,
                node_role,
                u64::try_from(path_level + 1)
                    .map_err(|_| TransparentStarkErrorV1::InvalidMerkleShape)?,
                u64::try_from(parent_index)
                    .map_err(|_| TransparentStarkErrorV1::InvalidMerkleShape)?,
                sibling,
                leaf,
            )?
        };
        index = parent_index;
    }
    if index != 0 || leaf != root {
        return Err(TransparentStarkErrorV1::InvalidMerkleShape);
    }
    Ok(())
}
/// Hash an unambiguous role-and-field frame.
#[cfg(test)]
pub(crate) fn goldilocks_frame_v1(
    context: TransparentStarkDigestContextV1,
    role: &[u8],
    level: u64,
    index: u64,
    fields: &[&[u8]],
) -> Result<GoldilocksDigest384V1, TransparentStarkErrorV1> {
    goldilocks_digest384_frame_v1(context, role, FRAME_PHASE_V1, level, index, 0, fields)
}
/// Stateful framed Fiat–Shamir transcript.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TransparentTranscriptV1 {
    context: TransparentStarkDigestContextV1,
    state: GoldilocksDigest384V1,
    challenge_counter: u64,
}
impl TransparentTranscriptV1 {
    /// Initialize with the engine suite, complete profile digest, and exact public-input digest.
    pub(crate) fn new(
        context: TransparentStarkDigestContextV1,
        engine_suite: &[u8],
        profile_digest: &GoldilocksDigest384V1,
        public_input_digest: &GoldilocksDigest384V1,
    ) -> Result<Self, TransparentStarkErrorV1> {
        if engine_suite.is_empty() {
            return Err(TransparentStarkErrorV1::FrameLengthOverflow);
        }
        let profile_digest = profile_digest.to_le_bytes();
        let public_input_digest = public_input_digest.to_le_bytes();
        Ok(Self {
            context,
            state: goldilocks_digest384_frame_v1(
                context,
                TRANSCRIPT_INIT_DOMAIN_V1,
                b"initialize",
                0,
                0,
                0,
                &[engine_suite, &profile_digest, &public_input_digest],
            )?,
            challenge_counter: 0,
        })
    }
    /// Absorb one labeled message and reset the local challenge counter.
    pub(crate) fn absorb(
        &mut self,
        label: &[u8],
        fields: &[&[u8]],
    ) -> Result<(), TransparentStarkErrorV1> {
        if label.is_empty() {
            return Err(TransparentStarkErrorV1::InvalidDigestDomain);
        }
        let message = goldilocks_digest384_frame_v1(
            self.context,
            b"transcript-message",
            label,
            0,
            0,
            self.challenge_counter,
            fields,
        )?;
        self.state = goldilocks_digest384_frame_v1(
            self.context,
            TRANSCRIPT_ABSORB_DOMAIN_V1,
            label,
            0,
            0,
            self.challenge_counter,
            &[&self.state.to_le_bytes(), &message.to_le_bytes()],
        )?;
        self.challenge_counter = 0;
        Ok(())
    }
    /// Current transcript state for query/grinding derivation.
    pub(crate) const fn state(&self) -> GoldilocksDigest384V1 {
        self.state
    }
    /// Exact catalog/protocol/profile context bound to this transcript.
    pub(crate) const fn context(&self) -> TransparentStarkDigestContextV1 {
        self.context
    }
    /// Derive one unbiased nonzero Goldilocks challenge.
    pub(crate) fn challenge_field(
        &mut self,
        label: &[u8],
    ) -> Result<GoldilocksFieldV1, TransparentStarkErrorV1> {
        for attempt in 0..MAX_FIELD_REJECTION_ATTEMPTS_V1 {
            let digest = goldilocks_digest384_frame_v1(
                self.context,
                TRANSCRIPT_CHALLENGE_DOMAIN_V1,
                label,
                0,
                attempt,
                self.challenge_counter,
                &[&self.state.to_le_bytes()],
            )?;
            let candidate = digest.words()[0];
            if let Some(field) = GoldilocksFieldV1::canonical(candidate)
                && field != GoldilocksFieldV1::ZERO
            {
                let accepted_counter = self.challenge_counter;
                self.challenge_counter = self
                    .challenge_counter
                    .checked_add(1)
                    .ok_or(TransparentStarkErrorV1::ChallengeSamplingExhausted)?;
                self.state = goldilocks_digest384_frame_v1(
                    self.context,
                    TRANSCRIPT_ABSORB_DOMAIN_V1,
                    label,
                    0,
                    0,
                    accepted_counter,
                    &[&self.state.to_le_bytes(), &digest.to_le_bytes()],
                )?;
                return Ok(field);
            }
        }
        Err(TransparentStarkErrorV1::ChallengeSamplingExhausted)
    }
    /// Derive one uniform challenge in the quartic Goldilocks extension.
    ///
    /// Four independent canonical digest lanes supply the fixed-order coefficients. FRI and
    /// polynomial identity theorems sample the whole challenge field, including zero; callers that
    /// need an invertible or out-of-domain value state that as a predicate via
    /// [`Self::challenge_fp4_where`].
    pub(crate) fn challenge_fp4(
        &mut self,
        label: &[u8],
    ) -> Result<GoldilocksFp4V1, TransparentStarkErrorV1> {
        self.challenge_fp4_where(label, |_| true)
    }
    /// Derive a uniform quartic challenge satisfying an additional deterministic public predicate.
    ///
    /// DEEP protocols use this to exclude their base and evaluation domains
    /// without absorbing a rejected candidate or introducing modulo bias.
    pub(crate) fn challenge_fp4_where(
        &mut self,
        label: &[u8],
        predicate: impl FnMut(GoldilocksFp4V1) -> bool,
    ) -> Result<GoldilocksFp4V1, TransparentStarkErrorV1> {
        let context = self.context;
        self.challenge_fp4_with_oracle_and_predicate(
            label,
            |state, label, counter, attempt| {
                goldilocks_digest384_frame_v1(
                    context,
                    TRANSCRIPT_FP4_CHALLENGE_DOMAIN_V1,
                    label,
                    0,
                    attempt,
                    counter,
                    &[&state.to_le_bytes()],
                )
            },
            predicate,
        )
    }
    #[cfg(test)]
    fn challenge_fp4_with_oracle(
        &mut self,
        label: &[u8],
        mut oracle: impl FnMut(
            GoldilocksDigest384V1,
            &[u8],
            u64,
            u64,
        ) -> Result<GoldilocksDigest384V1, TransparentStarkErrorV1>,
    ) -> Result<GoldilocksFp4V1, TransparentStarkErrorV1> {
        self.challenge_fp4_with_oracle_and_predicate(label, &mut oracle, |_| true)
    }
    fn challenge_fp4_with_oracle_and_predicate(
        &mut self,
        label: &[u8],
        mut oracle: impl FnMut(
            GoldilocksDigest384V1,
            &[u8],
            u64,
            u64,
        ) -> Result<GoldilocksDigest384V1, TransparentStarkErrorV1>,
        mut predicate: impl FnMut(GoldilocksFp4V1) -> bool,
    ) -> Result<GoldilocksFp4V1, TransparentStarkErrorV1> {
        for attempt in 0..MAX_FIELD_REJECTION_ATTEMPTS_V1 {
            let digest = oracle(self.state, label, self.challenge_counter, attempt)?;
            let words = digest.words();
            let field = GoldilocksFp4V1::canonical([words[0], words[1], words[2], words[3]])
                .expect("digest lanes are canonical Goldilocks elements");
            if predicate(field) {
                let accepted_counter = self.challenge_counter;
                self.challenge_counter = self
                    .challenge_counter
                    .checked_add(1)
                    .ok_or(TransparentStarkErrorV1::ChallengeSamplingExhausted)?;
                self.state = goldilocks_digest384_frame_v1(
                    self.context,
                    TRANSCRIPT_ABSORB_DOMAIN_V1,
                    label,
                    0,
                    0,
                    accepted_counter,
                    &[&self.state.to_le_bytes(), &digest.to_le_bytes()],
                )?;
                return Ok(field);
            }
        }
        Err(TransparentStarkErrorV1::ChallengeSamplingExhausted)
    }
}
/// Derive unique unbiased query indices for a power-of-two domain.
pub(crate) fn derive_unique_query_indices_v1(
    context: TransparentStarkDigestContextV1,
    seed: &GoldilocksDigest384V1,
    domain_size: usize,
    query_count: usize,
) -> Result<Vec<usize>, TransparentStarkErrorV1> {
    context.validate()?;
    if domain_size == 0
        || !domain_size.is_power_of_two()
        || query_count == 0
        || query_count > domain_size
    {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    let mut indices = Vec::new();
    indices
        .try_reserve_exact(query_count)
        .map_err(|_| TransparentStarkErrorV1::AllocationFailure)?;
    // A sparse swap table implements the first `query_count` steps of a Fisher--Yates shuffle
    // without allocating the complete domain. This gives every ordered set of distinct indices
    // the same probability and cannot exhibit coupon-collector exhaustion for dense queries.
    let mut swaps = BTreeMap::new();
    let mut counter = 0_u64;
    for query_number in 0..query_count {
        let remaining = domain_size
            .checked_sub(query_number)
            .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
        let offset =
            derive_bounded_query_offset_v1(context, seed, query_number, &mut counter, remaining)?;
        let draw = query_number
            .checked_add(offset)
            .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
        let selected = swaps.get(&draw).copied().unwrap_or(draw);
        let replacement = swaps.get(&query_number).copied().unwrap_or(query_number);
        swaps.insert(draw, replacement);
        indices.push(selected);
    }
    Ok(indices)
}
fn derive_bounded_query_offset_v1(
    context: TransparentStarkDigestContextV1,
    seed: &GoldilocksDigest384V1,
    query_number: usize,
    counter: &mut u64,
    bound: usize,
) -> Result<usize, TransparentStarkErrorV1> {
    if bound == 0 {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    let bound = bound as u128;
    let source_space = u128::from(GOLDILOCKS_MODULUS_V1);
    let acceptance_limit = source_space - source_space % bound;
    for _ in 0..MAX_QUERY_INDEX_REJECTION_ATTEMPTS_V1 {
        let digest = goldilocks_digest384_frame_v1(
            context,
            QUERY_INDEX_DOMAIN_V1,
            b"fisher-yates-offset",
            0,
            u64::try_from(query_number).map_err(|_| TransparentStarkErrorV1::InvalidDomain)?,
            *counter,
            &[
                &seed.to_le_bytes(),
                &u64::try_from(bound)
                    .map_err(|_| TransparentStarkErrorV1::InvalidDomain)?
                    .to_le_bytes(),
            ],
        )?;
        *counter = counter
            .checked_add(1)
            .ok_or(TransparentStarkErrorV1::QuerySamplingExhausted)?;
        let raw = u128::from(digest.words()[0]);
        if raw < acceptance_limit {
            return usize::try_from(raw % bound)
                .map_err(|_| TransparentStarkErrorV1::InvalidDomain);
        }
    }
    Err(TransparentStarkErrorV1::QuerySamplingExhausted)
}
/// Compute one binary FRI fold.
#[cfg(test)]
pub(crate) fn fri_fold_pair_v1(
    low: GoldilocksFieldV1,
    high: GoldilocksFieldV1,
    beta: GoldilocksFieldV1,
    x: GoldilocksFieldV1,
) -> Result<GoldilocksFieldV1, TransparentStarkErrorV1> {
    let inverse_x = x.inv().ok_or(TransparentStarkErrorV1::DivisionByZero)?;
    fri_fold_pair_with_inverse_x_v1(low, high, beta, inverse_x)
}
/// Compute one binary FRI fold when the caller already tracks `x^-1`.
///
/// Provers fold an entire multiplicative coset in order and can update the inverse point with one
/// multiplication per entry. Keeping that optimization here avoids duplicating the
/// consensus-critical fold equation in each relation-specific engine.
#[cfg(test)]
pub(crate) fn fri_fold_pair_with_inverse_x_v1(
    low: GoldilocksFieldV1,
    high: GoldilocksFieldV1,
    beta: GoldilocksFieldV1,
    inverse_x: GoldilocksFieldV1,
) -> Result<GoldilocksFieldV1, TransparentStarkErrorV1> {
    let inverse_two = GoldilocksFieldV1(2)
        .inv()
        .ok_or(TransparentStarkErrorV1::DivisionByZero)?;
    let even = low.add(high).mul(inverse_two);
    let odd = low.sub(high).mul(inverse_two).mul(inverse_x);
    Ok(even.add(beta.mul(odd)))
}
/// Compute one binary FRI fold over the quartic Goldilocks extension.
pub(crate) fn fri_fold_pair_fp4_v1(
    low: GoldilocksFp4V1,
    high: GoldilocksFp4V1,
    beta: GoldilocksFp4V1,
    x: GoldilocksFieldV1,
) -> Result<GoldilocksFp4V1, TransparentStarkErrorV1> {
    if x.0 >= GOLDILOCKS_MODULUS_V1 {
        return Err(TransparentStarkErrorV1::NonCanonicalField);
    }
    let inverse_x = x.inv().ok_or(TransparentStarkErrorV1::DivisionByZero)?;
    fri_fold_pair_with_inverse_x_fp4_v1(low, high, beta, inverse_x)
}
/// Compute one quartic-extension FRI fold with a tracked base-domain `x^-1`.
pub(crate) fn fri_fold_pair_with_inverse_x_fp4_v1(
    low: GoldilocksFp4V1,
    high: GoldilocksFp4V1,
    beta: GoldilocksFp4V1,
    inverse_x: GoldilocksFieldV1,
) -> Result<GoldilocksFp4V1, TransparentStarkErrorV1> {
    if !low.is_canonical()
        || !high.is_canonical()
        || !beta.is_canonical()
        || inverse_x.0 >= GOLDILOCKS_MODULUS_V1
    {
        return Err(TransparentStarkErrorV1::NonCanonicalField);
    }
    if inverse_x == GoldilocksFieldV1::ZERO {
        return Err(TransparentStarkErrorV1::DivisionByZero);
    }
    let inverse_two = GoldilocksFieldV1(2)
        .inv()
        .ok_or(TransparentStarkErrorV1::DivisionByZero)?;
    let even = low.add(high).mul_base(inverse_two);
    let odd = low.sub(high).mul_base(inverse_two).mul_base(inverse_x);
    Ok(even.add(beta.mul(odd)))
}
/// Check the entire terminal FRI polynomial against an exact degree bound.
#[cfg(test)]
pub(crate) fn ensure_fri_terminal_degree_v1(
    values: &[GoldilocksFieldV1],
    log_size: u8,
    degree_bound: usize,
) -> Result<(), TransparentStarkErrorV1> {
    let expected = 1_usize
        .checked_shl(u32::from(log_size))
        .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
    if values.len() != expected || degree_bound >= expected {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    let root = goldilocks_primitive_root_v1(log_size)?;
    let mut coefficients = values.to_vec();
    goldilocks_ifft_v1(&mut coefficients, root)?;
    if coefficients[degree_bound + 1..]
        .iter()
        .any(|coefficient| *coefficient != GoldilocksFieldV1::ZERO)
    {
        return Err(TransparentStarkErrorV1::FriDegree);
    }
    Ok(())
}
/// Check an entire quartic-extension FRI terminal against an exact degree.
pub(crate) fn ensure_fri_terminal_degree_fp4_v1(
    values: &[GoldilocksFp4V1],
    log_size: u8,
    degree_bound: usize,
) -> Result<(), TransparentStarkErrorV1> {
    let expected = 1_usize
        .checked_shl(u32::from(log_size))
        .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
    if values.len() != expected || degree_bound >= expected {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    if values.iter().any(|value| !value.is_canonical()) {
        return Err(TransparentStarkErrorV1::NonCanonicalField);
    }
    let root = goldilocks_primitive_root_v1(log_size)?;
    let mut coefficients = values.to_vec();
    goldilocks_fp4_ifft_v1(&mut coefficients, root)?;
    if coefficients[degree_bound + 1..]
        .iter()
        .any(|coefficient| *coefficient != GoldilocksFp4V1::ZERO)
    {
        return Err(TransparentStarkErrorV1::FriDegree);
    }
    Ok(())
}
/// Search for the smallest nonce meeting an exact leading-zero-bit target.
pub(crate) fn grind_nonce_v1(
    context: TransparentStarkDigestContextV1,
    transcript_seed: &GoldilocksDigest384V1,
    grinding_bits: u8,
) -> Result<u64, TransparentStarkErrorV1> {
    if grinding_bits > 63 {
        return Err(TransparentStarkErrorV1::InvalidGrinding);
    }
    if grinding_bits < GRINDING_PARALLEL_MIN_BITS_V1 {
        return grind_nonce_serial_v1(context, transcript_seed, grinding_bits);
    }
    first_nonce_in_ordered_parallel_chunks_v1(|nonce| {
        verify_grinding_nonce_v1(context, transcript_seed, grinding_bits, nonce).is_ok()
    })
    .ok_or(TransparentStarkErrorV1::InvalidGrinding)
}
fn first_nonce_in_ordered_parallel_chunks_v1<Accept>(accept: Accept) -> Option<u64>
where
    Accept: Fn(u64) -> bool + Sync,
{
    let mut chunk_start = 0_u64;
    loop {
        let chunk_end = chunk_start.saturating_add(GRINDING_PARALLEL_CHUNK_SIZE_V1 - 1);
        if let Some(nonce) = (chunk_start..=chunk_end)
            .into_par_iter()
            .filter(|nonce| accept(*nonce))
            .min()
        {
            return Some(nonce);
        }
        if chunk_end == u64::MAX {
            return None;
        }
        chunk_start = chunk_end + 1;
    }
}
fn grind_nonce_serial_v1(
    context: TransparentStarkDigestContextV1,
    transcript_seed: &GoldilocksDigest384V1,
    grinding_bits: u8,
) -> Result<u64, TransparentStarkErrorV1> {
    for nonce in 0..=u64::MAX {
        if verify_grinding_nonce_v1(context, transcript_seed, grinding_bits, nonce).is_ok() {
            return Ok(nonce);
        }
    }
    Err(TransparentStarkErrorV1::InvalidGrinding)
}
/// Verify a transcript grinding nonce.
pub(crate) fn verify_grinding_nonce_v1(
    context: TransparentStarkDigestContextV1,
    transcript_seed: &GoldilocksDigest384V1,
    grinding_bits: u8,
    nonce: u64,
) -> Result<(), TransparentStarkErrorV1> {
    if grinding_bits > 63 {
        return Err(TransparentStarkErrorV1::InvalidGrinding);
    }
    let digest = goldilocks_digest384_frame_v1(
        context,
        GRINDING_DOMAIN_V1,
        b"proof-of-work-nonce",
        0,
        nonce,
        0,
        &[&transcript_seed.to_le_bytes()],
    )?;
    if leading_zero_bits_v1(&digest.to_le_bytes()) < u32::from(grinding_bits) {
        return Err(TransparentStarkErrorV1::InvalidGrinding);
    }
    Ok(())
}
fn leading_zero_bits_v1(bytes: &[u8]) -> u32 {
    let mut count = 0_u32;
    for byte in bytes {
        if *byte == 0 {
            count += 8;
        } else {
            count += byte.leading_zeros();
            break;
        }
    }
    count
}
/// Strict fixed-shape proof reader.
pub(crate) struct ExactProofReaderV1<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> ExactProofReaderV1<'a> {
    /// Construct over one size-capped proof slice.
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    /// Read an exact byte array.
    pub(crate) fn take<const N: usize>(&mut self) -> Result<[u8; N], TransparentStarkErrorV1> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(TransparentStarkErrorV1::MalformedProof)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(TransparentStarkErrorV1::MalformedProof)?;
        self.offset = end;
        bytes
            .try_into()
            .map_err(|_| TransparentStarkErrorV1::MalformedProof)
    }
    /// Read big-endian `u16`.
    pub(crate) fn u16(&mut self) -> Result<u16, TransparentStarkErrorV1> {
        self.take().map(u16::from_be_bytes)
    }
    /// Read big-endian `u32`.
    pub(crate) fn u32(&mut self) -> Result<u32, TransparentStarkErrorV1> {
        self.take().map(u32::from_be_bytes)
    }
    /// Read big-endian `u64`.
    pub(crate) fn u64(&mut self) -> Result<u64, TransparentStarkErrorV1> {
        self.take().map(u64::from_be_bytes)
    }
    /// Read one canonical Goldilocks value.
    pub(crate) fn field(&mut self) -> Result<GoldilocksFieldV1, TransparentStarkErrorV1> {
        GoldilocksFieldV1::canonical(self.u64()?).ok_or(TransparentStarkErrorV1::NonCanonicalField)
    }
    /// Read one canonically encoded quartic-extension value.
    pub(crate) fn fp4(&mut self) -> Result<GoldilocksFp4V1, TransparentStarkErrorV1> {
        GoldilocksFp4V1::from_coefficients([
            self.field()?,
            self.field()?,
            self.field()?,
            self.field()?,
        ])
        .ok_or(TransparentStarkErrorV1::NonCanonicalField)
    }
    /// Require exact end-of-input.
    pub(crate) fn finish(self) -> Result<(), TransparentStarkErrorV1> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(TransparentStarkErrorV1::MalformedProof)
        }
    }
}
/// Append big-endian fixed integers to a canonical proof.
pub(crate) fn append_u16_v1(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_be_bytes());
}
/// Append big-endian fixed integers to a canonical proof.
pub(crate) fn append_u32_v1(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_be_bytes());
}
/// Append big-endian fixed integers to a canonical proof.
pub(crate) fn append_u64_v1(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_be_bytes());
}
/// Append one canonical quartic-extension value.
#[cfg_attr(not(any(test, feature = "zk-stark")), allow(dead_code))]
pub(crate) fn append_goldilocks_fp4_v1(bytes: &mut Vec<u8>, value: GoldilocksFp4V1) {
    bytes.extend_from_slice(&value.to_be_bytes());
}
#[cfg(test)]
mod tests {
    use super::*;
    use rand::{RngCore, SeedableRng as _, rngs::StdRng};
    const TEST_DIGEST_CONTEXT_V1: TransparentStarkDigestContextV1 =
        TransparentStarkDigestContextV1::new(
            PrivacyProtocolIdV1::PqMaspStarkV1,
            b"aggregate-test-profile-v1",
        );
    fn fp4(coefficients: [u64; 4]) -> GoldilocksFp4V1 {
        GoldilocksFp4V1::canonical(coefficients).expect("small canonical coefficients")
    }
    fn digest_from_fp4(value: GoldilocksFp4V1) -> GoldilocksDigest384V1 {
        let coefficients = value.coefficients().map(GoldilocksFieldV1::value);
        GoldilocksDigest384V1::new([
            coefficients[0],
            coefficients[1],
            coefficients[2],
            coefficients[3],
            0,
            0,
        ])
        .expect("Fp4 coefficients are canonical digest lanes")
    }
    fn test_transcript_v1() -> TransparentTranscriptV1 {
        let profile = GoldilocksDigest384V1::new([1; 6]).expect("profile digest");
        let public = GoldilocksDigest384V1::new([2; 6]).expect("public digest");
        TransparentTranscriptV1::new(TEST_DIGEST_CONTEXT_V1, b"fp4-suite", &profile, &public)
            .expect("transcript")
    }
    #[derive(Debug)]
    struct InjectedRngError;
    impl core::fmt::Display for InjectedRngError {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter.write_str("injected transparent-STARK RNG failure")
        }
    }
    struct FailingTryRng;
    impl TryRngCore for FailingTryRng {
        type Error = InjectedRngError;
        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Err(InjectedRngError)
        }
        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Err(InjectedRngError)
        }
        fn try_fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), Self::Error> {
            Err(InjectedRngError)
        }
    }
    struct MaxValueRng;
    impl RngCore for MaxValueRng {
        fn next_u32(&mut self) -> u32 {
            u32::MAX
        }
        fn next_u64(&mut self) -> u64 {
            u64::MAX
        }
        fn fill_bytes(&mut self, destination: &mut [u8]) {
            destination.fill(0xff);
        }
    }
    struct ZeroRng;
    impl RngCore for ZeroRng {
        fn next_u32(&mut self) -> u32 {
            0
        }
        fn next_u64(&mut self) -> u64 {
            0
        }
        fn fill_bytes(&mut self, destination: &mut [u8]) {
            destination.fill(0);
        }
    }
    #[test]
    fn zk_mask_geometry_uses_reduced_air_degree_and_coefficient_count_exactly() {
        let zk_ace =
            transparent_stark_zk_mask_geometry_v1(1, 4, 1, 108).expect("quadratic ZK-ACE AIR");
        assert_eq!(zk_ace.minimum_mask_coefficients, 332);
        assert_eq!(zk_ace.minimum_mask_degree, 331);
        let quadratic_ca =
            transparent_stark_zk_mask_geometry_v1(1, 4, 1, 60).expect("quadratic CA census");
        assert_eq!(quadratic_ca.minimum_mask_coefficients, 188);
        assert_eq!(quadratic_ca.minimum_mask_degree, 187);
        let cubic_ca = transparent_stark_zk_mask_geometry_v1(2, 4, 1, 60).expect("cubic CA AIR");
        assert_eq!(cubic_ca.minimum_mask_coefficients, 316);
        assert_eq!(cubic_ca.minimum_mask_degree, 315);
        let quartic_main =
            transparent_stark_zk_mask_geometry_v1(3, 4, 1, 60).expect("quartic main AIR");
        assert_eq!(quartic_main.minimum_mask_coefficients, 444);
        assert_eq!(quartic_main.minimum_mask_degree, 443);
        let deep_free =
            transparent_stark_zk_mask_geometry_v1(1, 4, 0, 60).expect("DEEP-free local proof");
        assert_eq!(deep_free.minimum_mask_coefficients, 180);
        assert_eq!(deep_free.minimum_mask_degree, 179);
        for invalid in [
            transparent_stark_zk_mask_geometry_v1(0, 4, 1, 60),
            transparent_stark_zk_mask_geometry_v1(1, 0, 1, 60),
            transparent_stark_zk_mask_geometry_v1(1, 4, 1, 0),
            transparent_stark_zk_mask_geometry_v1(usize::MAX, 4, 1, 60),
            transparent_stark_zk_mask_geometry_v1(1, usize::MAX, usize::MAX, 60),
        ] {
            assert_eq!(invalid, Err(TransparentStarkErrorV1::InvalidDomain));
        }
    }
    #[test]
    fn work_normalized_fiat_shamir_certificate_is_exact_and_fail_closed() {
        let boundary = checked_transparent_stark_work_security_v1(128, 129, 384, 252)
            .expect("the exact conservative boundary must pass");
        assert_eq!(boundary.target_bits, 128);
        assert_eq!(boundary.round_by_round_bits, 129);
        assert_eq!(boundary.random_oracle_bits, 384);
        assert_eq!(boundary.max_random_oracle_query_log2, 252);
        assert_eq!(
            checked_transparent_stark_work_security_v1(128, 128, 384, 252),
            Err(TransparentStarkErrorV1::InvalidDomain),
            "the RBR term must receive half of the target error budget"
        );
        assert_eq!(
            checked_transparent_stark_work_security_v1(128, 129, 384, 253),
            Err(TransparentStarkErrorV1::InvalidDomain),
            "the random-oracle term must receive the other half"
        );
        assert_eq!(
            checked_transparent_stark_work_security_v1(128, 129, 131, 0),
            Err(TransparentStarkErrorV1::InvalidDomain),
            "a digest too short for the split must fail closed"
        );
        assert_eq!(
            checked_transparent_stark_work_security_v1(0, 129, 384, 252),
            Err(TransparentStarkErrorV1::InvalidDomain)
        );
        assert_eq!(
            checked_transparent_stark_work_security_v1(u16::MAX, u16::MAX, u16::MAX, 0),
            Err(TransparentStarkErrorV1::InvalidDomain),
            "bit-count overflow must fail closed"
        );
    }
    #[test]
    fn goldilocks_fp4_is_an_irreducible_field_with_exact_arithmetic() {
        assert_eq!(GOLDILOCKS_MODULUS_V1 % 4, 1);
        assert_eq!(
            GOLDILOCKS_FP4_NONRESIDUE_V1.pow(u128::from((GOLDILOCKS_MODULUS_V1 - 1) / 2)),
            GoldilocksFieldV1(GOLDILOCKS_MODULUS_V1 - 1),
            "Euler's criterion must certify that 7 is a non-square"
        );
        let generator = fp4([0, 1, 0, 0]);
        assert_eq!(
            generator.pow(4),
            GoldilocksFp4V1::from_base(GOLDILOCKS_FP4_NONRESIDUE_V1)
        );
        let left = fp4([1, 2, 3, 4]);
        let middle = fp4([5, 6, 7, 8]);
        let right = fp4([9, 10, 11, 12]);
        assert_eq!(left.mul(middle), fp4([432, 380, 258, 60]));
        assert_eq!(
            left.inv().expect("KAT input is nonzero"),
            fp4([
                14_840_051_427_074_313_514,
                8_965_911_045_340_978_602,
                2_500_729_783_671_062_999,
                974_321_935_410_605_745,
            ])
        );
        assert_eq!(left.mul(middle).mul(right), left.mul(middle.mul(right)));
        assert_eq!(
            left.mul(middle.add(right)),
            left.mul(middle).add(left.mul(right))
        );
        assert_eq!(left.add(left.neg()), GoldilocksFp4V1::ZERO);
        assert_eq!(GoldilocksFp4V1::ZERO.inv(), None);
        for c0 in 0..4 {
            for c1 in 0..4 {
                for c2 in 0..4 {
                    for c3 in 0..4 {
                        let value = fp4([c0, c1, c2, c3]);
                        if value != GoldilocksFp4V1::ZERO {
                            let inverse =
                                value.inv().expect("every nonzero Fp4 value is invertible");
                            assert_eq!(value.mul(inverse), GoldilocksFp4V1::ONE);
                            assert_eq!(inverse.mul(value), GoldilocksFp4V1::ONE);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn division_free_goldilocks_arithmetic_matches_u128_reference() {
        let modulus = u128::from(GOLDILOCKS_MODULUS_V1);
        let boundary_values = [
            0,
            1,
            u128::from(GOLDILOCKS_EPSILON_V1 - 1),
            u128::from(GOLDILOCKS_EPSILON_V1),
            u128::from(GOLDILOCKS_EPSILON_V1 + 1),
            modulus - 1,
            modulus,
            modulus + 1,
            u128::from(u64::MAX),
            u128::from(u64::MAX) + 1,
            (1_u128 << 96) - 1,
            1_u128 << 96,
            (1_u128 << 127) - 1,
            1_u128 << 127,
            u128::MAX - 1,
            u128::MAX,
        ];
        for value in boundary_values {
            let reduced = GoldilocksFieldV1::reduce(value);
            assert_eq!(u128::from(reduced.0), value % modulus, "value={value}");
            assert!(reduced.0 < GOLDILOCKS_MODULUS_V1);
        }
        let boundary_residues = [
            0,
            1,
            2,
            GOLDILOCKS_EPSILON_V1 - 1,
            GOLDILOCKS_EPSILON_V1,
            GOLDILOCKS_EPSILON_V1 + 1,
            GOLDILOCKS_MODULUS_V1 / 2,
            GOLDILOCKS_MODULUS_V1 - GOLDILOCKS_EPSILON_V1,
            GOLDILOCKS_MODULUS_V1 - 2,
            GOLDILOCKS_MODULUS_V1 - 1,
        ];
        for left in boundary_residues {
            for right in boundary_residues {
                let left_field = GoldilocksFieldV1(left);
                let right_field = GoldilocksFieldV1(right);
                let sum = left_field.add(right_field);
                let product = left_field.mul(right_field);
                assert_eq!(
                    u128::from(sum.0),
                    (u128::from(left) + u128::from(right)) % modulus,
                    "left={left}, right={right}"
                );
                assert_eq!(
                    u128::from(product.0),
                    (u128::from(left) * u128::from(right)) % modulus,
                    "left={left}, right={right}"
                );
                assert!(sum.0 < GOLDILOCKS_MODULUS_V1);
                assert!(product.0 < GOLDILOCKS_MODULUS_V1);
            }
        }
        let mut rng = StdRng::from_seed([0x6D; 32]);
        for sample in 0..200_000 {
            let value = (u128::from(rng.next_u64()) << 64) | u128::from(rng.next_u64());
            let reduced = GoldilocksFieldV1::reduce(value);
            assert_eq!(
                u128::from(reduced.0),
                value % modulus,
                "arbitrary reduction sample {sample}"
            );
            let left = rng.next_u64() % GOLDILOCKS_MODULUS_V1;
            let right = rng.next_u64() % GOLDILOCKS_MODULUS_V1;
            let sum = GoldilocksFieldV1(left).add(GoldilocksFieldV1(right));
            let product = GoldilocksFieldV1(left).mul(GoldilocksFieldV1(right));
            assert_eq!(
                u128::from(sum.0),
                (u128::from(left) + u128::from(right)) % modulus,
                "random addition sample {sample}"
            );
            assert_eq!(
                u128::from(product.0),
                (u128::from(left) * u128::from(right)) % modulus,
                "random multiplication sample {sample}"
            );
        }
    }
    #[test]
    fn goldilocks_fp4_codec_is_fixed_order_canonical_and_exact() {
        let value = fp4([1, 2, 3, GOLDILOCKS_MODULUS_V1 - 1]);
        let bytes = value.to_be_bytes();
        assert_eq!(bytes.len(), 32);
        assert_eq!(GoldilocksFp4V1::canonical_be_bytes(bytes), Some(value));
        assert_eq!(value.coefficients()[0], GoldilocksFieldV1::ONE);
        assert_eq!(
            GoldilocksFp4V1::from_coefficients(value.coefficients()),
            Some(value)
        );
        let mut noncanonical = bytes;
        noncanonical[..8].copy_from_slice(&GOLDILOCKS_MODULUS_V1.to_be_bytes());
        assert_eq!(GoldilocksFp4V1::canonical_be_bytes(noncanonical), None);
        assert_eq!(
            GoldilocksFp4V1::canonical([0, 0, 0, GOLDILOCKS_MODULUS_V1]),
            None
        );
        let mut encoded = Vec::new();
        append_goldilocks_fp4_v1(&mut encoded, value);
        let mut reader = ExactProofReaderV1::new(&encoded);
        assert_eq!(reader.fp4().expect("canonical Fp4"), value);
        reader.finish().expect("exact Fp4 byte stream");
        let mut encoded_noncanonical = encoded;
        encoded_noncanonical[8..16].copy_from_slice(&GOLDILOCKS_MODULUS_V1.to_be_bytes());
        assert_eq!(
            ExactProofReaderV1::new(&encoded_noncanonical).fp4(),
            Err(TransparentStarkErrorV1::NonCanonicalField)
        );
    }
    #[test]
    fn goldilocks_fp4_fft_coset_terminal_and_fold_are_exact() {
        for log_size in 0..=10 {
            let root = goldilocks_primitive_root_v1(log_size).expect("primitive root");
            let size = 1_usize << log_size;
            let mut values = (0..size)
                .map(|index| {
                    let index = index as u64;
                    fp4([
                        index + 1,
                        index.wrapping_mul(3) + 2,
                        index.wrapping_mul(5) + 3,
                        index.wrapping_mul(7) + 4,
                    ])
                })
                .collect::<Vec<_>>();
            let original = values.clone();
            goldilocks_fp4_fft_v1(&mut values, root).expect("Fp4 FFT");
            goldilocks_fp4_ifft_v1(&mut values, root).expect("Fp4 IFFT");
            assert_eq!(values, original);
        }
        let root = goldilocks_primitive_root_v1(4).expect("root");
        let coefficients = vec![fp4([1, 2, 3, 4]), fp4([5, 6, 7, 8]), fp4([9, 10, 11, 12])];
        let evaluations =
            goldilocks_fp4_evaluate_coset_v1(&coefficients, 16, root, GoldilocksFieldV1::ONE)
                .expect("Fp4 evaluation");
        ensure_fri_terminal_degree_fp4_v1(&evaluations, 4, 2).expect("quadratic terminal");
        let mut high = evaluations;
        high[3] = high[3].add(GoldilocksFp4V1::ONE);
        assert_eq!(
            ensure_fri_terminal_degree_fp4_v1(&high, 4, 2),
            Err(TransparentStarkErrorV1::FriDegree)
        );
        let low = fp4([11, 12, 13, 14]);
        let high = fp4([19, 20, 21, 22]);
        let beta = fp4([23, 24, 25, 26]);
        let point = GoldilocksFieldV1(29);
        assert_eq!(
            fri_fold_pair_fp4_v1(low, high, beta, point).expect("Fp4 fold"),
            fri_fold_pair_with_inverse_x_fp4_v1(
                low,
                high,
                beta,
                point.inv().expect("nonzero point")
            )
            .expect("optimized Fp4 fold")
        );
        assert_eq!(
            fri_fold_pair_fp4_v1(low, high, beta, GoldilocksFieldV1::ZERO),
            Err(TransparentStarkErrorV1::DivisionByZero)
        );
        assert_eq!(
            fri_fold_pair_fp4_v1(low, high, beta, GoldilocksFieldV1(GOLDILOCKS_MODULUS_V1)),
            Err(TransparentStarkErrorV1::NonCanonicalField),
            "a malformed domain point must not be reduced before inversion"
        );
        assert_eq!(
            goldilocks_fp4_ifft_v1(
                &mut [GoldilocksFp4V1::ONE],
                GoldilocksFieldV1(GOLDILOCKS_MODULUS_V1)
            ),
            Err(TransparentStarkErrorV1::InvalidDomain),
            "a malformed inverse-FFT root must fail before exponentiation"
        );
    }
    #[test]
    fn goldilocks_fp4_transcript_and_rng_sampling_fail_closed() {
        let mut first = test_transcript_v1();
        first.absorb(b"root", &[b"commitment"]).expect("absorb");
        let challenge = first.challenge_fp4(b"beta").expect("Fp4 challenge");
        assert!(challenge.is_canonical());
        let mut replay = test_transcript_v1();
        replay.absorb(b"root", &[b"commitment"]).expect("absorb");
        assert_eq!(replay.challenge_fp4(b"beta").expect("replay"), challenge);
        let mut zero_allowed = test_transcript_v1();
        let mut attempts = 0_u64;
        assert_eq!(
            zero_allowed.challenge_fp4_with_oracle(b"beta", |_, _, _, _| {
                attempts += 1;
                Ok(GoldilocksDigest384V1::default())
            }),
            Ok(GoldilocksFp4V1::ZERO),
            "FRI challenges are uniform over the complete extension field"
        );
        assert_eq!(attempts, 1);
        let mut predicate_retry = test_transcript_v1();
        let rejected = GoldilocksFp4V1::ZERO;
        let accepted = fp4([9, 10, 11, 12]);
        let mut attempts = 0_u64;
        let mut retry_frames = Vec::new();
        let sampled = predicate_retry
            .challenge_fp4_with_oracle_and_predicate(
                b"deep-point",
                |_, label, counter, attempt| {
                    assert_eq!(label, b"deep-point");
                    retry_frames.push((counter, attempt));
                    attempts += 1;
                    Ok(if attempts == 1 {
                        digest_from_fp4(rejected)
                    } else {
                        digest_from_fp4(accepted)
                    })
                },
                |candidate| candidate != GoldilocksFp4V1::ZERO,
            )
            .expect("nonzero predicate accepts the second candidate");
        assert_eq!(attempts, 2);
        assert_eq!(retry_frames, [(0, 0), (0, 1)]);
        assert_eq!(sampled, accepted);
        let mut direct = test_transcript_v1();
        assert_eq!(
            direct
                .challenge_fp4_with_oracle(b"deep-point", |_, _, _, _| {
                    Ok(digest_from_fp4(accepted))
                })
                .expect("direct accepted candidate"),
            accepted
        );
        assert_eq!(
            predicate_retry.state(),
            direct.state(),
            "a rejected DEEP point must not be absorbed into the transcript"
        );
        let mut predicate_exhausted = test_transcript_v1();
        let initial_state = predicate_exhausted.state();
        let mut exhausted_frames = Vec::new();
        assert_eq!(
            predicate_exhausted.challenge_fp4_with_oracle_and_predicate(
                b"deep-point",
                |_, label, counter, attempt| {
                    assert_eq!(label, b"deep-point");
                    exhausted_frames.push((counter, attempt));
                    Ok(digest_from_fp4(rejected))
                },
                |candidate| candidate != GoldilocksFp4V1::ZERO,
            ),
            Err(TransparentStarkErrorV1::ChallengeSamplingExhausted)
        );
        assert_eq!(
            exhausted_frames,
            (0..MAX_FIELD_REJECTION_ATTEMPTS_V1)
                .map(|attempt| (0, attempt))
                .collect::<Vec<_>>()
        );
        assert_eq!(predicate_exhausted.state(), initial_state);
        let mut first_rng = StdRng::from_seed([0x44; 32]);
        let mut replay_rng = StdRng::from_seed([0x44; 32]);
        assert_eq!(
            random_goldilocks_fp4_v1(&mut first_rng).expect("random Fp4"),
            random_goldilocks_fp4_v1(&mut replay_rng).expect("replayed random Fp4")
        );
        assert_ne!(
            random_nonzero_goldilocks_fp4_v1(&mut first_rng).expect("nonzero random Fp4"),
            GoldilocksFp4V1::ZERO
        );
        assert_eq!(
            random_goldilocks_fp4_v1(&mut FailingTryRng),
            Err(TransparentStarkErrorV1::RandomnessUnavailable)
        );
        assert_eq!(
            random_goldilocks_fp4_v1(&mut MaxValueRng),
            Err(TransparentStarkErrorV1::RandomnessUnavailable)
        );
        assert_eq!(
            random_nonzero_goldilocks_fp4_v1(&mut ZeroRng),
            Err(TransparentStarkErrorV1::RandomnessUnavailable)
        );
    }
    #[test]
    fn fft_roundtrips_every_small_power_of_two_domain() {
        assert_eq!(
            GoldilocksFieldV1(GOLDILOCKS_MODULUS_V1).inv(),
            None,
            "inversion must never canonicalize a malformed field wrapper"
        );
        assert_eq!(
            goldilocks_ifft_v1(
                &mut [GoldilocksFieldV1::ONE],
                GoldilocksFieldV1(GOLDILOCKS_MODULUS_V1)
            ),
            Err(TransparentStarkErrorV1::InvalidDomain)
        );
        for log_size in 0..=12 {
            let root = goldilocks_primitive_root_v1(log_size).expect("primitive root");
            let size = 1_usize << log_size;
            let mut values = (0..size)
                .map(|index| GoldilocksFieldV1::reduce((index as u128 + 7).pow(3)))
                .collect::<Vec<_>>();
            let original = values.clone();
            goldilocks_fft_v1(&mut values, root).expect("FFT");
            goldilocks_ifft_v1(&mut values, root).expect("IFFT");
            assert_eq!(values, original);
        }
        let mut wrong_order = vec![GoldilocksFieldV1::ONE; 8];
        assert_eq!(
            goldilocks_fft_v1(&mut wrong_order, GoldilocksFieldV1::ONE),
            Err(TransparentStarkErrorV1::InvalidDomain)
        );
        let root = goldilocks_primitive_root_v1(3).expect("primitive root");
        let mut noncanonical = vec![GoldilocksFieldV1::ZERO; 8];
        noncanonical[3] = GoldilocksFieldV1(GOLDILOCKS_MODULUS_V1);
        assert_eq!(
            goldilocks_fft_v1(&mut noncanonical, root),
            Err(TransparentStarkErrorV1::NonCanonicalField)
        );
    }
    #[test]
    fn masked_trace_lde_is_randomized_and_preserves_degree_capacity() {
        let base = (0..16).map(GoldilocksFieldV1).collect::<Vec<_>>();
        let mut first_rng = StdRng::from_seed([0x11; 32]);
        let mut second_rng = StdRng::from_seed([0x22; 32]);
        let first = masked_trace_lde_column_v1(&base, 4, 7, 7, &mut first_rng).expect("first mask");
        let mut replay_rng = StdRng::from_seed([0x11; 32]);
        let replay_mask = sample_trace_mask_v1(7, &mut replay_rng).expect("replayable mask");
        let replay = masked_trace_lde_column_with_mask_v1(&base, 4, 7, replay_mask.coefficients())
            .expect("replayed mask");
        let coefficients =
            masked_trace_coefficients_with_mask_v1(&base, 4, replay_mask.coefficients())
                .expect("retained masked coefficients");
        assert_eq!(coefficients.len(), 24);
        assert_eq!(
            masked_trace_coefficients_on_coset_v1(&coefficients, 4, 7)
                .expect("retained coefficient replay"),
            replay
        );
        let native_root = goldilocks_primitive_root_v1(4).expect("native root");
        for (index, expected) in base.iter().copied().enumerate() {
            let point = native_root.pow(index as u128);
            let actual = coefficients
                .iter()
                .rev()
                .copied()
                .fold(GoldilocksFieldV1::ZERO, |value, coefficient| {
                    value.mul(point).add(coefficient)
                });
            assert_eq!(actual, expected, "native row {index}");
        }
        assert_eq!(first, replay);
        let second =
            masked_trace_lde_column_v1(&base, 4, 7, 7, &mut second_rng).expect("second mask");
        assert_eq!(first.len(), 128);
        assert_ne!(first, second);
        assert_eq!(
            masked_trace_lde_column_with_mask_v1(&base, 4, 7, &[]),
            Err(TransparentStarkErrorV1::InvalidDomain)
        );
        assert_eq!(
            masked_trace_lde_column_with_mask_v1(&base, 4, 4, replay_mask.coefficients(),),
            Err(TransparentStarkErrorV1::InvalidDomain)
        );
        assert_eq!(
            masked_trace_lde_column_v1(&base, 4, 4, 0, &mut first_rng),
            Err(TransparentStarkErrorV1::InvalidDomain)
        );
        assert_eq!(
            masked_trace_coefficients_with_mask_v1(&base[..15], 4, replay_mask.coefficients()),
            Err(TransparentStarkErrorV1::InvalidDomain)
        );
        assert_eq!(
            masked_trace_coefficients_with_mask_v1(&base, 4, &[]),
            Err(TransparentStarkErrorV1::InvalidDomain)
        );
        assert_eq!(
            masked_trace_coefficients_with_mask_v1(&base, 4, &[GoldilocksFieldV1(u64::MAX)],),
            Err(TransparentStarkErrorV1::NonCanonicalField)
        );
        assert_eq!(
            masked_trace_coefficients_on_coset_v1(&coefficients, 4, 4),
            Err(TransparentStarkErrorV1::InvalidDomain)
        );
        assert_eq!(
            masked_trace_coefficients_on_coset_v1(&[], 4, 7),
            Err(TransparentStarkErrorV1::InvalidDomain)
        );
        assert_eq!(
            masked_trace_coefficients_on_coset_v1(
                &[GoldilocksFieldV1(GOLDILOCKS_MODULUS_V1)],
                4,
                7,
            ),
            Err(TransparentStarkErrorV1::NonCanonicalField)
        );
        assert_eq!(
            goldilocks_evaluate_coset_v1(
                &[GoldilocksFieldV1(GOLDILOCKS_MODULUS_V1)],
                128,
                goldilocks_primitive_root_v1(7).expect("evaluation root"),
                GoldilocksFieldV1(GOLDILOCKS_GENERATOR_V1),
            ),
            Err(TransparentStarkErrorV1::NonCanonicalField)
        );
    }
    #[test]
    fn merkle_paths_bind_domain_index_leaf_order_and_depth() {
        let node_role = b"iroha:test:transparent-stark:node:v1";
        let leaves = (0..8)
            .map(|index| {
                goldilocks_frame_v1(
                    TEST_DIGEST_CONTEXT_V1,
                    b"iroha:test:transparent-stark:leaf:v1",
                    0,
                    u64::from(index),
                    &[&[index]],
                )
                .expect("leaf hash")
            })
            .collect::<Vec<_>>();
        let tree =
            GoldilocksMerkleTreeV1::from_leaves(leaves.clone(), TEST_DIGEST_CONTEXT_V1, node_role)
                .expect("tree");
        for (index, leaf) in leaves.iter().copied().enumerate() {
            verify_goldilocks_merkle_path_v1(
                TEST_DIGEST_CONTEXT_V1,
                node_role,
                tree.root(),
                leaf,
                index,
                &tree.path(index).expect("path"),
                3,
            )
            .expect("opening");
        }
        let mut path = tree.path(3).expect("path");
        let mut mutated = path[0].words();
        mutated[0] = if mutated[0] + 1 == GOLDILOCKS_MODULUS_V1 {
            0
        } else {
            mutated[0] + 1
        };
        path[0] = GoldilocksDigest384V1::new(mutated).expect("mutation remains canonical");
        assert_eq!(
            verify_goldilocks_merkle_path_v1(
                TEST_DIGEST_CONTEXT_V1,
                node_role,
                tree.root(),
                leaves[3],
                3,
                &path,
                3,
            ),
            Err(TransparentStarkErrorV1::InvalidMerkleShape)
        );
        assert_eq!(
            verify_goldilocks_merkle_path_v1(
                TEST_DIGEST_CONTEXT_V1,
                b"other",
                tree.root(),
                leaves[3],
                3,
                &path,
                3,
            ),
            Err(TransparentStarkErrorV1::InvalidMerkleShape)
        );
        let oversized_domain: &'static [u8] =
            Box::leak(vec![0_u8; usize::from(u16::MAX) + 1].into_boxed_slice());
        assert!(matches!(
            GoldilocksMerkleTreeV1::from_leaves(
                vec![GoldilocksDigest384V1::default(); 2],
                TEST_DIGEST_CONTEXT_V1,
                oversized_domain,
            ),
            Err(TransparentStarkErrorV1::InvalidMerkleShape)
        ));
        assert_eq!(
            verify_goldilocks_merkle_path_v1(
                TEST_DIGEST_CONTEXT_V1,
                oversized_domain,
                tree.root(),
                leaves[3],
                3,
                &path,
                3,
            ),
            Err(TransparentStarkErrorV1::InvalidMerkleShape)
        );
    }

    #[test]
    fn merkle_root_and_paths_are_identical_across_rayon_widths() {
        let node_role = b"iroha:test:transparent-stark:parallel-node:v1";
        let leaves = (0_u64..1_024)
            .map(|index| {
                goldilocks_digest384_frame_v1(
                    TEST_DIGEST_CONTEXT_V1,
                    b"iroha:test:transparent-stark:parallel-leaf:v1",
                    b"parallel-determinism",
                    0,
                    index,
                    0,
                    &[&index.to_be_bytes()],
                )
                .expect("leaf hash")
            })
            .collect::<Vec<_>>();
        let build = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("private test thread pool")
                .install(|| {
                    GoldilocksMerkleTreeV1::from_leaves(
                        leaves.clone(),
                        TEST_DIGEST_CONTEXT_V1,
                        node_role,
                    )
                    .expect("parallel Merkle tree")
                })
        };
        let serial = build(1);
        let parallel = build(4);
        let mut reference_levels = vec![leaves.clone()];
        while reference_levels.last().map_or(0, Vec::len) > 1 {
            let parent_level = u64::try_from(reference_levels.len()).expect("small tree depth");
            let previous = reference_levels.last().expect("reference Merkle level");
            let next = previous
                .chunks_exact(2)
                .enumerate()
                .map(|(index, pair)| {
                    goldilocks_merkle_node_v1(
                        TEST_DIGEST_CONTEXT_V1,
                        node_role,
                        parent_level,
                        u64::try_from(index).expect("small parent index"),
                        pair[0],
                        pair[1],
                    )
                    .expect("reference serial parent")
                })
                .collect::<Vec<_>>();
            reference_levels.push(next);
        }
        let reference = GoldilocksMerkleTreeV1 {
            levels: reference_levels,
        };
        assert_eq!(serial.root(), reference.root());
        assert_eq!(serial.root(), parallel.root());
        for index in [0, 1, 2, 511, 512, 1_022, 1_023] {
            assert_eq!(serial.path(index), reference.path(index));
            assert_eq!(serial.path(index), parallel.path(index));
        }
    }
    #[test]
    fn transcript_is_framed_ordered_and_deterministic() {
        let profile = GoldilocksDigest384V1::new([1; 6]).expect("profile digest");
        let public = GoldilocksDigest384V1::new([2; 6]).expect("public digest");
        let mut first =
            TransparentTranscriptV1::new(TEST_DIGEST_CONTEXT_V1, b"suite", &profile, &public)
                .expect("transcript");
        let mut second = first;
        first.absorb(b"root", &[b"ab", b"c"]).expect("absorb");
        second.absorb(b"root", &[b"a", b"bc"]).expect("absorb");
        assert_ne!(first.state(), second.state());
        let challenge = first.challenge_field(b"alpha").expect("challenge");
        let mut replay =
            TransparentTranscriptV1::new(TEST_DIGEST_CONTEXT_V1, b"suite", &profile, &public)
                .expect("transcript");
        replay.absorb(b"root", &[b"ab", b"c"]).expect("absorb");
        assert_eq!(
            replay.challenge_field(b"alpha").expect("challenge"),
            challenge
        );
    }
    #[test]
    fn digest_binds_protocol_profile_role_phase_level_index_counter_and_lanes() {
        let digest = goldilocks_digest384_frame_v1(
            TEST_DIGEST_CONTEXT_V1,
            b"trace-tree",
            b"leaf",
            3,
            5,
            7,
            &[b"payload"],
        )
        .expect("digest");
        let mutations = [
            goldilocks_digest384_frame_v1(
                TransparentStarkDigestContextV1::new(
                    PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                    b"aggregate-test-profile-v1",
                ),
                b"trace-tree",
                b"leaf",
                3,
                5,
                7,
                &[b"payload"],
            ),
            goldilocks_digest384_frame_v1(
                TransparentStarkDigestContextV1::new(
                    PrivacyProtocolIdV1::PqMaspStarkV1,
                    b"different-profile-v1",
                ),
                b"trace-tree",
                b"leaf",
                3,
                5,
                7,
                &[b"payload"],
            ),
            goldilocks_digest384_frame_v1(
                TEST_DIGEST_CONTEXT_V1,
                b"composition-tree",
                b"leaf",
                3,
                5,
                7,
                &[b"payload"],
            ),
            goldilocks_digest384_frame_v1(
                TEST_DIGEST_CONTEXT_V1,
                b"trace-tree",
                b"node",
                3,
                5,
                7,
                &[b"payload"],
            ),
            goldilocks_digest384_frame_v1(
                TEST_DIGEST_CONTEXT_V1,
                b"trace-tree",
                b"leaf",
                4,
                5,
                7,
                &[b"payload"],
            ),
            goldilocks_digest384_frame_v1(
                TEST_DIGEST_CONTEXT_V1,
                b"trace-tree",
                b"leaf",
                3,
                6,
                7,
                &[b"payload"],
            ),
            goldilocks_digest384_frame_v1(
                TEST_DIGEST_CONTEXT_V1,
                b"trace-tree",
                b"leaf",
                3,
                5,
                8,
                &[b"payload"],
            ),
        ];
        for mutation in mutations {
            assert_ne!(mutation.expect("mutated domain"), digest);
        }
        let lanes = digest.words().into_iter().collect::<BTreeSet<_>>();
        assert_eq!(lanes.len(), 6, "all six independently keyed lanes differ");
    }
    #[test]
    fn query_indices_are_unique_deterministic_and_in_domain() {
        let seed = GoldilocksDigest384V1::new([9; 6]).expect("canonical seed");
        let first = derive_unique_query_indices_v1(TEST_DIGEST_CONTEXT_V1, &seed, 1 << 12, 56)
            .expect("queries");
        let second = derive_unique_query_indices_v1(TEST_DIGEST_CONTEXT_V1, &seed, 1 << 12, 56)
            .expect("queries");
        assert_eq!(first, second);
        assert!(first.iter().all(|index| *index < 1 << 12));
        assert_eq!(
            first.iter().copied().collect::<BTreeSet<_>>().len(),
            first.len()
        );
    }
    #[test]
    fn dense_query_sampling_succeeds_for_the_former_zero_seed_exhaustion() {
        let seed = GoldilocksDigest384V1::default();
        let indices = derive_unique_query_indices_v1(TEST_DIGEST_CONTEXT_V1, &seed, 2, 2)
            .expect("dense queries");
        assert_eq!(indices.len(), 2);
        let indices = indices.into_iter().collect::<BTreeSet<_>>();
        assert!(indices.contains(&0));
        assert!(indices.contains(&1));
    }
    #[test]
    fn query_indices_remain_unique_and_in_range_through_full_domains() {
        let seed = GoldilocksDigest384V1::new([0xa5; 6]).expect("canonical seed");
        for domain_size in [1_usize, 2, 4, 8, 16, 64, 256] {
            for query_count in [1, domain_size.div_ceil(2), domain_size] {
                let indices = derive_unique_query_indices_v1(
                    TEST_DIGEST_CONTEXT_V1,
                    &seed,
                    domain_size,
                    query_count,
                )
                .expect("valid query shape");
                assert_eq!(indices.len(), query_count);
                assert!(indices.iter().all(|index| *index < domain_size));
                assert_eq!(
                    indices.iter().copied().collect::<BTreeSet<_>>().len(),
                    query_count
                );
            }
        }
    }
    #[test]
    fn fri_terminal_check_rejects_high_degree_values() {
        let root = goldilocks_primitive_root_v1(4).expect("root");
        let linear = (0..16)
            .scan(GoldilocksFieldV1::ONE, |point, _| {
                let value = GoldilocksFieldV1(7).add(GoldilocksFieldV1(3).mul(*point));
                *point = point.mul(root);
                Some(value)
            })
            .collect::<Vec<_>>();
        ensure_fri_terminal_degree_v1(&linear, 4, 1).expect("linear");
        let mut high = linear;
        high[3] = high[3].add(GoldilocksFieldV1::ONE);
        assert_eq!(
            ensure_fri_terminal_degree_v1(&high, 4, 1),
            Err(TransparentStarkErrorV1::FriDegree)
        );
        let low = GoldilocksFieldV1(11);
        let high = GoldilocksFieldV1(19);
        let beta = GoldilocksFieldV1(23);
        let point = GoldilocksFieldV1(29);
        assert_eq!(
            fri_fold_pair_v1(low, high, beta, point).expect("fold"),
            fri_fold_pair_with_inverse_x_v1(low, high, beta, point.inv().expect("nonzero point"))
                .expect("optimized fold")
        );
        assert_eq!(
            fri_fold_pair_v1(low, high, beta, GoldilocksFieldV1::ZERO),
            Err(TransparentStarkErrorV1::DivisionByZero)
        );
    }
    #[test]
    fn batch_inversion_rejects_empty_and_noncanonical_inputs() {
        assert_eq!(
            goldilocks_batch_invert_v1(&mut []),
            Err(TransparentStarkErrorV1::InvalidDomain)
        );
        let mut noncanonical = [
            GoldilocksFieldV1::ONE,
            GoldilocksFieldV1(GOLDILOCKS_MODULUS_V1),
        ];
        assert_eq!(
            goldilocks_batch_invert_v1(&mut noncanonical),
            Err(TransparentStarkErrorV1::NonCanonicalField)
        );
    }
    #[test]
    fn grinding_and_exact_reader_fail_closed() {
        let seed = GoldilocksDigest384V1::new([0x42; 6]).expect("canonical seed");
        let nonce = grind_nonce_v1(TEST_DIGEST_CONTEXT_V1, &seed, 8).expect("grind");
        verify_grinding_nonce_v1(TEST_DIGEST_CONTEXT_V1, &seed, 8, nonce).expect("valid nonce");
        if nonce > 0 {
            assert_eq!(
                verify_grinding_nonce_v1(TEST_DIGEST_CONTEXT_V1, &seed, 8, nonce - 1),
                Err(TransparentStarkErrorV1::InvalidGrinding)
            );
        }
        let mut bytes = Vec::new();
        append_u16_v1(&mut bytes, 7);
        append_u32_v1(&mut bytes, 11);
        append_u64_v1(&mut bytes, 13);
        append_u64_v1(&mut bytes, GOLDILOCKS_MODULUS_V1 - 1);
        let mut reader = ExactProofReaderV1::new(&bytes);
        assert_eq!(reader.u16().expect("u16"), 7);
        assert_eq!(reader.u32().expect("u32"), 11);
        assert_eq!(reader.u64().expect("u64"), 13);
        assert_eq!(
            reader.field().expect("field"),
            GoldilocksFieldV1(GOLDILOCKS_MODULUS_V1 - 1)
        );
        reader.finish().expect("exact end");
        let mut noncanonical = Vec::new();
        append_u64_v1(&mut noncanonical, GOLDILOCKS_MODULUS_V1);
        assert_eq!(
            ExactProofReaderV1::new(&noncanonical).field(),
            Err(TransparentStarkErrorV1::NonCanonicalField)
        );
    }
    #[test]
    fn grinding_scheduler_advances_chunks_and_is_identical_across_rayon_widths() {
        let first_match = GRINDING_PARALLEL_CHUNK_SIZE_V1 + 17;
        let later_match = GRINDING_PARALLEL_CHUNK_SIZE_V1 + 29;
        assert!(first_match > GRINDING_PARALLEL_CHUNK_SIZE_V1 - 1);
        let search = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("private grinding test pool")
                .install(|| {
                    first_nonce_in_ordered_parallel_chunks_v1(|nonce| {
                        nonce == first_match || nonce == later_match
                    })
                })
        };
        assert_eq!(search(1), Some(first_match));
        assert_eq!(search(4), Some(first_match));
    }
}
