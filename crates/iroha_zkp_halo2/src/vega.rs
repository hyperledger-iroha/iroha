//! Native field and curve adapters for the pinned Microsoft Vega profile.
//!
//! Vega's canonical engine uses the T256 group. Its scalar field is exactly
//! the P-256 base field, which lets the mDL circuit ingest issuer-key
//! coordinates without non-native reduction. This module keeps that identity
//! explicit and exposes only canonical, non-reducing encodings to callers.
//!
//! The protocol source is Microsoft `vega-prover` commit
//! `c0ee259053cd12eaf43ed71b5cde375452b3ee4d`, licensed under MIT.
use core::{
    fmt,
    ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign},
};
use halo2curves::{
    ff::{Field, FromUniformBytes, PrimeField},
    t256::Fq,
};
use thiserror::Error;
#[path = "vega/algebra.rs"]
mod algebra;
#[path = "vega/canonical_mc_exact.rs"]
mod canonical_mc;
#[path = "vega/circuit.rs"]
mod circuit;
#[path = "vega/commitment.rs"]
mod commitment;
#[path = "vega/curve.rs"]
mod curve;
#[path = "vega/date.rs"]
mod date;
#[path = "vega/engine.rs"]
mod engine;
#[path = "vega/figure9.rs"]
mod figure9;
#[path = "vega/figure9_layout.rs"]
mod figure9_layout;
#[path = "vega/hyrax.rs"]
mod hyrax;
#[path = "vega/masked_relaxed.rs"]
mod masked_relaxed;
#[path = "vega/microsoft_mc.rs"]
mod microsoft_mc;
#[path = "vega/nifs.rs"]
mod nifs;
#[path = "vega/p256.rs"]
mod p256;
#[path = "vega/r1cs.rs"]
mod r1cs;
#[path = "vega/sha256.rs"]
mod sha256;
#[path = "vega/spartan.rs"]
mod spartan;
#[path = "vega/sponge.rs"]
mod sponge;
#[path = "vega/sumcheck.rs"]
mod sumcheck;
#[path = "vega/transcript.rs"]
mod transcript;
#[path = "vega/wire.rs"]
mod wire;
#[path = "vega/zk_ams.rs"]
mod zk_ams;
pub(super) use curve::{
    VEGA_T256_BASE_MODULUS_BE_V1, VegaCurveError, VegaT256PointV1, derive_t256_generators_v1,
};
pub use engine::{
    MAX_VEGA_PROVER_RELEASE_MEMORY_CEILING_BYTES_V1, MAX_VEGA_PROVER_WORKERS_V1,
    VEGA_EXISTING_CREDENTIAL_PROTOCOL_LABEL_V1, VEGA_INTERNAL_TRANSCRIPT_PERSONA_V1,
    VEGA_MDL_ACTION_INDEX_V1, VEGA_MDL_CANONICAL_RELATION_DIGEST_V1,
    VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1, VEGA_MDL_COMPILED_PROFILE_DIGEST_V1,
    VEGA_MDL_COMPILED_PROFILE_MANIFEST_V1, VEGA_MDL_MC_STEP_COUNT_V1,
    VEGA_PROVER_SHARED_MEMORY_BOUND_BYTES_V1, VegaMdlProofContextV1, VegaMdlProofDimensionsV1,
    VegaMdlProofErrorV1, VegaMdlProverConfigV1, VegaRandomSourceErrorV1, VegaRandomSourceV1,
    prove_vega_mdl_figure9_v1, vega_mdl_canonical_relation_digest_v1,
    vega_mdl_compiled_profile_digest_v1, vega_mdl_proof_dimensions_v1, vega_mdl_verifier_digest_v1,
    vega_microsoft_fixture_conformance_v1, verify_vega_mdl_figure9_v1,
};
pub use figure9::{
    VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1, VegaMdlFigure9ErrorV1, VegaMdlFigure9WitnessV1,
    validate_vega_mdl_figure9_encoding_v1, validate_vega_mdl_figure9_relation_v1,
};
pub use masked_relaxed::{MaskedRelaxedRandomErrorV1, MaskedRelaxedRandomSourceV1};
pub(super) use transcript::{VegaTranscriptError, VegaTranscriptV1};
pub(super) use wire::{VegaPointWireV1, VegaScalarWireV1};
pub use zk_ams::{
    MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1, ZK_AMS_ACTION_INDEX_V1,
    ZK_AMS_ADMISSION_PUBLIC_INPUTS_V1, ZK_AMS_PHC_CANONICAL_PAYLOAD_BYTES_V1,
    ZkAmsAdmissionPublicInputV1, ZkAmsAdmissionRelationDimensionsV1, ZkAmsAdmissionRelationErrorV1,
    ZkAmsAdmissionRelationWitnessV1, ZkAmsMaskedProverConfigV1, ZkAmsMkheErrorV1,
    ZkAmsMkheReadinessV1, ZkAmsMkheReleaseManifestV1, ZkAmsProofContextV1,
    prove_zk_ams_admission_relation_v1, verify_zk_ams_admission_relation_v1,
    zk_ams_admission_relation_dimensions_v1, zk_ams_compiled_profile_digest_v1,
    zk_ams_mkhe_manifest_digest_v1, zk_ams_mkhe_readiness_digest_v1, zk_ams_mkhe_readiness_v1,
    zk_ams_mkhe_release_manifest_v1, zk_ams_release_candidate_profile_digest_v1,
    zk_ams_t256_generator_digest_v1,
};

/// Exact canonical COSE `Sig_structure` width in the released Figure 9 relation.
pub const VEGA_MDL_ISSUER_AUTHENTICATION_SIG_STRUCTURE_BYTES_V1: usize = 368;
/// Exact tagged ISO 18013-5 MSO payload width embedded in the `Sig_structure`.
pub const VEGA_MDL_MSO_PAYLOAD_BYTES_V1: usize = 348;
/// Exact tagged `IssuerSignedItemBytes` width for the private birth date.
pub const VEGA_MDL_BIRTH_DATE_ISSUER_SIGNED_ITEM_BYTES_V1: usize = 92;
/// Exact randomizer width inside the birth-date signed item.
pub const VEGA_MDL_BIRTH_RANDOM_BYTES_V1: usize = 16;
/// Exact `YYYY-MM-DD` text width parsed by the released relation.
pub const VEGA_MDL_FULL_DATE_TEXT_BYTES_V1: usize = 10;
/// Exact `YYYY-MM-DDTHH:MM:SSZ` text width parsed by the released relation.
pub const VEGA_MDL_RFC3339_UTC_SECONDS_TEXT_BYTES_V1: usize = 20;
/// Lowest trusted UTC presentation year admitted by the released relation.
pub const VEGA_MDL_MIN_PRESENTATION_YEAR_V1: u16 = 1_970;
/// Highest presentation year for which a later four-digit `validUntil` exists.
pub const VEGA_MDL_MAX_PRESENTATION_YEAR_V1: u16 = 9_998;
/// Lowest non-degenerate public age threshold admitted by the released relation.
pub const VEGA_MDL_MIN_AGE_THRESHOLD_YEARS_V1: u8 = 1;
/// Highest achievable public age threshold admitted by the released relation.
pub const VEGA_MDL_MAX_AGE_THRESHOLD_YEARS_V1: u8 = 150;
/// Tight first-release cap for one canonical Norito Vega proof.
///
/// A 512 KiB ceiling leaves room for the exact 368-byte Figure 9 relation and Norito framing while
/// preventing this engine from inheriting the much broader per-action opaque-byte allowance.
pub const MAX_VEGA_PROOF_BYTES_V1: usize = 512 * 1024;
/// Big-endian modulus of the canonical T256 scalar field.
///
/// This is also the base-field modulus of NIST P-256.
pub const VEGA_T256_SCALAR_MODULUS_BE_V1: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
];
struct ZeroizingVegaScalarBytesV1([u8; 32]);
impl Drop for ZeroizingVegaScalarBytesV1 {
    fn drop(&mut self) {
        let bytes = core::hint::black_box(&mut self.0);
        bytes.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *bytes);
    }
}
/// Failure while translating canonical Vega field material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum VegaFieldError {
    /// The supplied big-endian integer is not smaller than the T256 scalar modulus.
    #[error("integer is not a canonical T256 scalar")]
    NonCanonicalScalar,
    /// The zero scalar does not have a multiplicative inverse.
    #[error("cannot invert the zero T256 scalar")]
    InversionOfZero,
}
/// Canonical T256 scalar used by Vega public inputs and proof-system algebra.
///
/// Construction is deliberately non-reducing: byte strings at or above the modulus are rejected
/// rather than silently mapped into the field. Secret scalars must be converted to explicit wire
/// types before crossing a serialization boundary:
///
/// ```compile_fail
/// use iroha_zkp_halo2::vega::VegaT256ScalarV1;
/// use norito::codec::Encode as _;
///
/// let secret = VegaT256ScalarV1::from_u64(42);
/// let _encoded = secret.encode();
/// ```
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct VegaT256ScalarV1(Fq);
impl VegaT256ScalarV1 {
    /// Return the additive identity.
    #[must_use]
    pub const fn zero() -> Self {
        Self(Fq::ZERO)
    }
    /// Return the multiplicative identity.
    #[must_use]
    pub const fn one() -> Self {
        Self(Fq::ONE)
    }
    /// Parse one canonical 32-byte big-endian scalar without modular reduction.
    ///
    /// # Errors
    ///
    /// Returns [`VegaFieldError::NonCanonicalScalar`] when `bytes` represents
    /// an integer greater than or equal to the scalar modulus.
    pub fn from_be_bytes_exact(bytes: [u8; 32]) -> Result<Self, VegaFieldError> {
        Self::from_be_bytes_exact_ref(&bytes)
    }
    /// Parse a borrowed canonical big-endian scalar while wiping the sole
    /// little-endian representation scratch on every exit.
    pub(crate) fn from_be_bytes_exact_ref(bytes: &[u8; 32]) -> Result<Self, VegaFieldError> {
        if bytes >= &VEGA_T256_SCALAR_MODULUS_BE_V1 {
            return Err(VegaFieldError::NonCanonicalScalar);
        }
        // `halo2curves` 0.9 exposes the P-256 base-field representation in
        // little-endian order. Keep that implementation detail behind this
        // explicitly big-endian Vega boundary.
        let mut repr = ZeroizingVegaScalarBytesV1(*bytes);
        repr.0.reverse();
        let value = Option::<Fq>::from(Fq::from_repr(repr.0.into()))
            .ok_or(VegaFieldError::NonCanonicalScalar)?;
        Ok(Self(value))
    }
    /// Parse one canonical 32-byte little-endian proof scalar without modular reduction.
    ///
    /// # Errors
    ///
    /// Returns [`VegaFieldError::NonCanonicalScalar`] when `bytes` represents
    /// an integer greater than or equal to the scalar modulus.
    pub fn from_le_bytes_exact(mut bytes: [u8; 32]) -> Result<Self, VegaFieldError> {
        bytes.reverse();
        Self::from_be_bytes_exact(bytes)
    }
    /// Reduce an exact 64-byte little-endian uniform string as specified by
    /// the pinned Vega Fiat--Shamir transcript.
    #[must_use]
    pub fn from_uniform_le_bytes(bytes: [u8; 64]) -> Self {
        Self::from_uniform_le_bytes_ref(&bytes)
    }
    /// Reduce a borrowed exact 64-byte little-endian uniform string.
    ///
    /// Secret-entropy owners should use this form so scalar reduction does not
    /// first create an unmanaged by-value copy of their zeroized buffer.
    #[must_use]
    pub fn from_uniform_le_bytes_ref(bytes: &[u8; 64]) -> Self {
        Self(Fq::from_uniform_bytes(bytes))
    }
    /// Construct a scalar from an unsigned 64-bit integer.
    #[must_use]
    pub fn from_u64(value: u64) -> Self {
        Self(Fq::from(value))
    }
    /// Return the exact canonical 32-byte big-endian representation.
    #[must_use]
    pub fn to_be_bytes(self) -> [u8; 32] {
        let mut bytes: [u8; 32] = self.0.to_repr().into();
        bytes.reverse();
        bytes
    }
    /// Return the exact canonical 32-byte little-endian proof encoding.
    #[must_use]
    pub fn to_le_bytes(self) -> [u8; 32] {
        let mut bytes = [0_u8; 32];
        self.write_le_bytes_ref(&mut bytes);
        bytes
    }
    /// Write a borrowed scalar directly into a caller-owned canonical
    /// little-endian proof-encoding slot.
    fn write_le_bytes_ref(&self, destination: &mut [u8; 32]) {
        *destination = self.0.to_repr().into();
    }
    /// Return whether this field element is zero.
    #[must_use]
    pub fn is_zero(self) -> bool {
        bool::from(self.0.is_zero())
    }
    /// Return the multiplicative inverse.
    ///
    /// # Errors
    ///
    /// Returns [`VegaFieldError::InversionOfZero`] for the additive identity.
    pub fn inverse(self) -> Result<Self, VegaFieldError> {
        Option::<Fq>::from(self.0.invert())
            .map(Self)
            .ok_or(VegaFieldError::InversionOfZero)
    }
    /// Square this scalar.
    #[must_use]
    pub fn square(self) -> Self {
        Self(self.0.square())
    }
    /// Replace this scalar instance with exact zero using a safe best-effort wipe.
    ///
    /// This scalar is [`Copy`]; callers must separately clear every independent copy that contains
    /// secret material. Rust also does not guarantee erasure of compiler-created temporaries.
    pub fn clear_secret(&mut self) {
        *self = Self::zero();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *self);
    }
}
impl Default for VegaT256ScalarV1 {
    fn default() -> Self {
        Self::zero()
    }
}
impl Add for VegaT256ScalarV1 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}
impl AddAssign for VegaT256ScalarV1 {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}
impl Sub for VegaT256ScalarV1 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}
impl SubAssign for VegaT256ScalarV1 {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}
impl Mul for VegaT256ScalarV1 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}
impl MulAssign for VegaT256ScalarV1 {
    fn mul_assign(&mut self, rhs: Self) {
        self.0 *= rhs.0;
    }
}
impl Neg for VegaT256ScalarV1 {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}
impl fmt::Debug for VegaT256ScalarV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("VegaT256ScalarV1(REDACTED)")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use halo2curves::ff::PrimeField;
    #[test]
    fn t256_scalar_modulus_is_exactly_the_p256_base_modulus() {
        assert_eq!(
            Fq::MODULUS,
            "0xffffffff00000001000000000000000000000000ffffffffffffffffffffffff"
        );
        let mut below = VEGA_T256_SCALAR_MODULUS_BE_V1;
        below[31] -= 1;
        let parsed = VegaT256ScalarV1::from_be_bytes_exact(below).expect("q - 1 is canonical");
        assert_eq!(parsed.to_be_bytes(), below);
        assert_eq!(
            VegaT256ScalarV1::from_be_bytes_exact(VEGA_T256_SCALAR_MODULUS_BE_V1),
            Err(VegaFieldError::NonCanonicalScalar)
        );
        assert_eq!(
            VegaT256ScalarV1::from_be_bytes_exact([0xff; 32]),
            Err(VegaFieldError::NonCanonicalScalar)
        );
    }
    #[test]
    fn t256_scalar_big_endian_boundary_does_not_reduce() {
        for value in [0_u64, 1, 255, 256, u32::MAX.into(), u64::MAX] {
            let scalar = VegaT256ScalarV1::from_u64(value);
            let mut expected = [0_u8; 32];
            expected[24..].copy_from_slice(&value.to_be_bytes());
            assert_eq!(scalar.to_be_bytes(), expected);
            assert_eq!(VegaT256ScalarV1::from_be_bytes_exact(expected), Ok(scalar));
        }
    }
    #[test]
    fn t256_scalar_clear_secret_replaces_nonzero_with_exact_zero() {
        let mut secret = VegaT256ScalarV1::from_be_bytes_exact([0x5a; 32])
            .expect("fixture is below the T256 scalar modulus");
        assert!(!secret.is_zero());
        secret.clear_secret();
        assert!(secret.is_zero());
        assert_eq!(secret, VegaT256ScalarV1::zero());
        assert_eq!(secret.to_be_bytes(), [0; 32]);
    }
    #[test]
    fn t256_scalar_debug_does_not_expose_secret_material() {
        let secret = VegaT256ScalarV1::from_u64(0x0123_4567_89ab_cdef);
        let rendered = format!("{secret:?}");
        assert_eq!(rendered, "VegaT256ScalarV1(REDACTED)");
        assert!(!rendered.contains("0123456789abcdef"));
    }
}
