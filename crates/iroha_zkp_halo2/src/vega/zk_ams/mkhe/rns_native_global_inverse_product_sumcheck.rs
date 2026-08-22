//! Compact, masked global inverse-product sum-check for the native RNS path.
//!
//! This private kernel replaces 8,102 independent 65,536-gate product cores
//! with one transcript-bound random linear combination and one 29-round cubic
//! sum-check.  It is deliberately not a lookup-membership proof: it proves
//! only that every already-committed inverse plane satisfies
//! `(z - A[p, v]) * U[p, v] = 1`, except with the explicitly accounted
//! batching and sum-check errors.
//!
//! The physical table has 32,408 planes of 16,384 values.  The 360 padding
//! planes in the `2^15` plane domain are not silently changed to zero
//! residuals.  They have the fixed public opening `A = 0`, `U = z^-1`, so the
//! literal padded-domain relation `(z - A) * U - 1 = 0` remains true.  Their
//! commitment contribution is derived from the fixed T256 basis and `z`; no
//! prover-selected padding commitment or opening exists.
//!
//! Transcript order is fixed as follows:
//! 1. all `A` commitments, the dedicated pre-`z` inverse-product mask
//!    commitment, and the unique `z`;
//! 2. all `U` commitments;
//! 3. the nonzero batching challenge `rho`;
//! 4. each masked cubic message, immediately followed by its nonzero round
//!    challenge;
//! 5. the verifier-derived folded `A`/`U` commitments and the canonical T256
//!    generalized-Bulletproof endpoint proof.
//! The enclosing post-`z` token hashes this proof as its residual, so that
//! token's binding is intentionally excluded from every challenge preimage.
//! It enters only the successor token's hashes after proof verification.
//!
//! The first 87 coordinates of that dedicated commitment contain three field
//! elements per round under the same 16,384-coordinate basis.  It is distinct
//! from the global-lookup sum-check mask (which uses a 1,024-coordinate basis
//! and remains owned by its later stage).  The honest builder zero-pads the
//! unused suffix; acceptance does not claim a value for that irrelevant
//! suffix.  If `h = 1/2`, round `j` adds
//! `h*carry - h*(m1+m2+m3) + m1*t + m2*t^2 + m3*t^3`.
//! This preserves the sum-check telescope and makes the three transmitted
//! coefficients information-theoretically independent.  The endpoint proof
//! binds the terminal carry to the precommitted mask opening.
//!
//! The source trait is replayable but move-only at the proof entry.  Its
//! repeated reads are a bounded-memory implementation detail, not a soundness
//! assumption: the final generalized-Bulletproof opening binds the random
//! multilinear endpoint to the authenticated physical commitments.

use core::{fmt, marker::PhantomData};

use crate::{
    generalized_bulletproof::{
        ArithmeticCircuitStatement, ArithmeticCircuitWitness, GeneralizedBulletproofErrorV1,
        LinComb, ProofRandomSource, ProofScalar, ProofSuite, ProverTranscript, Variable,
        VectorCommitmentOpening, VerifierTranscript, multiexp,
    },
    vega::{
        VEGA_T256_SCALAR_MODULUS_BE_V1, VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
        bulletproof_t256::{ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1, ZkAmsT256BulletproofSuiteV1},
        sponge::Keccak256,
    },
};

const VERSION_V1: u8 = 1;
const FLAGS_V1: u8 = 0;
const MAGIC_V1: [u8; 4] = *b"ZGIS";
const DIGEST_BYTES_V1: usize = 32;
const POINT_BYTES_V1: usize = 33;
const SCALAR_BYTES_V1: usize = 32;
const HEADER_BYTES_V1: usize = 40;
const ACTIVE_PLANES_V1: usize = 32_408;
const PADDED_PLANES_V1: usize = 1 << 15;
const PADDING_PLANES_V1: usize = PADDED_PLANES_V1 - ACTIVE_PLANES_V1;
const COORDINATES_V1: usize = 1 << 14;
const COORDINATE_BITS_V1: usize = 14;
const PLANE_BITS_V1: usize = 15;
const SUMCHECK_ROUNDS_V1: usize = COORDINATE_BITS_V1 + PLANE_BITS_V1;
const SUMCHECK_DEGREE_V1: usize = 3;
const MASK_SCALARS_PER_ROUND_V1: usize = 3;
const MASK_SCALARS_V1: usize = SUMCHECK_ROUNDS_V1 * MASK_SCALARS_PER_ROUND_V1;
const MESSAGE_SCALARS_PER_ROUND_V1: usize = 3;
const MESSAGE_SCALARS_V1: usize = SUMCHECK_ROUNDS_V1 * MESSAGE_SCALARS_PER_ROUND_V1;
const MESSAGE_BYTES_V1: usize = MESSAGE_SCALARS_V1 * SCALAR_BYTES_V1;
const ENDPOINT_VECTOR_COMMITMENTS_V1: usize = 3;
const ENDPOINT_GATES_V1: usize = 1;
const ENDPOINT_CONSTRAINTS_V1: usize = 3;
const ENDPOINT_LOG_GATES_V1: usize = COORDINATE_BITS_V1;
const ENDPOINT_NI_V1: usize = 2 + 2 * (ENDPOINT_VECTOR_COMMITMENTS_V1 / 2);
const ENDPOINT_L_POLYNOMIALS_V1: usize = ENDPOINT_NI_V1 + 2;
const ENDPOINT_T_POLYNOMIALS_V1: usize = 2 * ENDPOINT_L_POLYNOMIALS_V1 - 1;
const ENDPOINT_FIXED_POINTS_V1: usize = 3 + ENDPOINT_T_POLYNOMIALS_V1 - 1;
const ENDPOINT_IPA_POINTS_V1: usize = 2 * ENDPOINT_LOG_GATES_V1;
const ENDPOINT_POINTS_V1: usize = ENDPOINT_FIXED_POINTS_V1 + ENDPOINT_IPA_POINTS_V1;
const ENDPOINT_SCALARS_V1: usize = 5;
const ENDPOINT_CORE_BYTES_V1: usize =
    ENDPOINT_POINTS_V1 * POINT_BYTES_V1 + ENDPOINT_SCALARS_V1 * SCALAR_BYTES_V1;
const CODEC_DIGEST_BYTES_V1: usize = DIGEST_BYTES_V1;
const PARENT_RESIDUAL_CAP_BYTES_V1: usize =
    super::RNS_NATIVE_GLOBAL_LOOKUP_POST_Z_RESIDUAL_MAX_BYTES_V1;
const MIN_DOWNSTREAM_RESIDUAL_BYTES_V1: usize = 1;
const OWNED_WIRE_BYTES_V1: usize =
    HEADER_BYTES_V1 + MESSAGE_BYTES_V1 + ENDPOINT_CORE_BYTES_V1 + CODEC_DIGEST_BYTES_V1;
pub(super) const RNS_NATIVE_GLOBAL_INVERSE_PRODUCT_RESIDUAL_MAX_BYTES_V1: usize =
    PARENT_RESIDUAL_CAP_BYTES_V1 - OWNED_WIRE_BYTES_V1;
const MIN_WIRE_BYTES_V1: usize = OWNED_WIRE_BYTES_V1 + MIN_DOWNSTREAM_RESIDUAL_BYTES_V1;
const MAX_CHALLENGE_ATTEMPTS_V1: u8 = 128;
const RHO_CHALLENGE_ORDINAL_V1: u32 = 0;

const MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-inverse-product.manifest";
const COMMITMENT_SET_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-inverse-product.commitments";
const RHO_TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-inverse-product.rho-transcript";
const SUMCHECK_TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-inverse-product.sumcheck-transcript";
const ENDPOINT_TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-inverse-product.endpoint-transcript";
const CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-inverse-product.challenge";
const RHO_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-inverse-product.rho-digest";
const CODEC_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-global-inverse-product.codec";
const RESIDUAL_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-inverse-product.residual";
const BINDING_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-global-inverse-product.binding";
const GEOMETRY_LANGUAGE_V1: &[u8] = b"flat-index=p*16384+v;v-bits-first-little-endian-14;p-bits-second-little-endian-15;active-p=0..32407;padding-p=32408..32767;padding-A=0;padding-U=z^-1";
const ROLE_ORDER_LANGUAGE_V1: &[u8] = b"A/U-plane-order:D-low[5848],S-low[5848],Delta[5848],small-positive[1032],small-negative[1032],q-digit[column-major,6400],q-complement[column-major,6400]";
const STREAMING_LANGUAGE_V1: &[u8] = b"move-only-replay-source;coordinate-prefix-folds-little-endian;RAM=O(16384+32768)-field-elements;no-530972672-cell-materialization;replay-consistency-is-not-trusted-by-verifier;random-endpoint-is-commitment-bound";
const RELATION_LANGUAGE_V1: &[u8] = b"rho-nonzero-after-all-A-U-commitments;R_rho=MLE_i(rho^i);F=R_rho*((z-A)*U-1);sum-over-{0,1}^29-F=0;cubic-individual-degree;endpoint-folds-existing-plane-commitments";
const MASK_LANGUAGE_V1: &[u8] = b"dedicated-inverse-product-mask-distinct-from-global-lookup-mask;87-pre-z-random-scalar-prefix-plus-independent-Pedersen-blinding-under-16384-basis;unused-suffix-is-not-claimed;per-round=(m1,m2,m3);h=1/2;mask(t)=h*carry-h*(m1+m2+m3)+m1*t+m2*t^2+m3*t^3;terminal-weights=h^(28-j)*(r_j-h,r_j^2-h,r_j^3-h)";
const SOUNDNESS_LANGUAGE_V1: &[u8] = b"per-fresh-transcript:batching-error<=(2^29-1)/(pT-1),sumcheck-error<=29*3/(pT-1),plus-at-most-48*2^-256-for-512-bit-reduction-and-nonzero-conditioning-across-30-sumcheck-and-18-endpoint-GBP-draws;challenge-exhaustion-fails-closed;Fiat-Shamir-adversary-incurs-standard-Keccak-ROM-query-loss;endpoint-binding-under-T256-DL-and-generalized-BP-transcript";
const TRANSCRIPT_LANGUAGE_V1: &[u8] = b"pre-z:A-and-mask;derive-z;post-z:U;derive-rho;for-j=0..28:absorb-(constant,quadratic,cubic)-then-derive-r_j;derive-folded-A-U;verify-one-T256-generalized-BP-core;exclude-current-frame-residual-and-binding-from-all-challenges;bind-them-only-after-verification";

const INVERSE_PRODUCT_RELATION_VERIFIED_V1: bool = true;
const LOOKUP_MEMBERSHIP_VERIFIED_V1: bool = false;
const CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1: bool = false;
const RELEASE_READY_V1: bool = false;

const _: () = {
    assert!(PADDING_PLANES_V1 == 360);
    assert!(ACTIVE_PLANES_V1 * COORDINATES_V1 == 530_972_672);
    assert!(PADDED_PLANES_V1 * COORDINATES_V1 == 1 << 29);
    assert!(SUMCHECK_ROUNDS_V1 == 29);
    assert!(MASK_SCALARS_V1 == 87);
    assert!(MESSAGE_SCALARS_V1 == 87);
    assert!(MESSAGE_BYTES_V1 == 2_784);
    assert!(ENDPOINT_NI_V1 == 4);
    assert!(ENDPOINT_L_POLYNOMIALS_V1 == 6);
    assert!(ENDPOINT_T_POLYNOMIALS_V1 == 11);
    assert!(ENDPOINT_FIXED_POINTS_V1 == 13);
    assert!(ENDPOINT_IPA_POINTS_V1 == 28);
    assert!(ENDPOINT_POINTS_V1 == 41);
    assert!(ENDPOINT_CORE_BYTES_V1 == 1_513);
    assert!(OWNED_WIRE_BYTES_V1 == 4_369);
    assert!(MIN_WIRE_BYTES_V1 == 4_370);
    assert!(RNS_NATIVE_GLOBAL_INVERSE_PRODUCT_RESIDUAL_MAX_BYTES_V1 == 110_115);
    assert!(MIN_WIRE_BYTES_V1 <= PARENT_RESIDUAL_CAP_BYTES_V1);
    assert!(INVERSE_PRODUCT_RELATION_VERIFIED_V1);
    assert!(!LOOKUP_MEMBERSHIP_VERIFIED_V1);
    assert!(!CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1);
    assert!(!RELEASE_READY_V1);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum RnsNativeGlobalInverseProductErrorV1 {
    ProofCapExceeded,
    InvalidHeader,
    InvalidGeometry,
    InvalidContext,
    InvalidPoint,
    InvalidScalar,
    InvalidSumcheck,
    InvalidEndpoint,
    InvalidIntegrity,
    ChallengeExhausted,
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "the confidential production replay source remains deliberately uninhabited"
        )
    )]
    SourceUnavailable,
    ArithmeticOverflow,
    ResourceExhausted,
}

impl fmt::Display for RnsNativeGlobalInverseProductErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeGlobalInverseProductErrorV1 {}

impl From<GeneralizedBulletproofErrorV1> for RnsNativeGlobalInverseProductErrorV1 {
    fn from(_: GeneralizedBulletproofErrorV1) -> Self {
        Self::InvalidEndpoint
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct KernelGeometryV1 {
    active_planes: usize,
    padded_planes: usize,
    coordinates: usize,
    coordinate_bits: usize,
    plane_bits: usize,
}

impl KernelGeometryV1 {
    const PRODUCTION: Self = Self {
        active_planes: ACTIVE_PLANES_V1,
        padded_planes: PADDED_PLANES_V1,
        coordinates: COORDINATES_V1,
        coordinate_bits: COORDINATE_BITS_V1,
        plane_bits: PLANE_BITS_V1,
    };

    fn validate_v1(self) -> Result<(), RnsNativeGlobalInverseProductErrorV1> {
        if self.active_planes == 0
            || self.active_planes >= self.padded_planes
            || !self.padded_planes.is_power_of_two()
            || !self.coordinates.is_power_of_two()
            || self.padded_planes != 1_usize << self.plane_bits
            || self.coordinates != 1_usize << self.coordinate_bits
            || self.rounds_v1() == 0
        {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
        }
        Ok(())
    }

    const fn rounds_v1(self) -> usize {
        self.coordinate_bits + self.plane_bits
    }

    fn mask_scalars_v1(self) -> Result<usize, RnsNativeGlobalInverseProductErrorV1> {
        self.rounds_v1()
            .checked_mul(MASK_SCALARS_PER_ROUND_V1)
            .ok_or(RnsNativeGlobalInverseProductErrorV1::ArithmeticOverflow)
    }
}

/// Replay-only source for the secret active-plane openings and the one
/// pre-`z` sum-check-mask opening.
///
/// `prove_kernel_v1` consumes this owner.  Implementations must overwrite the
/// complete output slices on every call with the requested plane after
/// binding the little-endian coordinate variables in `coordinate_prefix`.
/// The output length is exactly `16_384 >> coordinate_prefix.len()` for the
/// production geometry.  This lets a confidential spool retain each round's
/// half-sized fold instead of replaying all raw values.  Value-only replay
/// deliberately cannot return Pedersen masks, so intermediate passes create
/// no ignored secret copies.  The sole full-opening method writes the original
/// physical commitment masks into caller-owned zeroizing slots.  The
/// inverse-product-mask opening is destructive and must contain three
/// independently uniform pre-`z` scalars per round plus an independent
/// uniform Pedersen blinding; its unused commitment suffix is irrelevant.
pub(super) trait RnsNativeGlobalInverseProductOpeningSourceV1 {
    fn replay_active_plane_values_v1(
        &mut self,
        ordinal: usize,
        coordinate_prefix: &[Scalar],
        a_values: &mut [Scalar],
        u_values: &mut [Scalar],
    ) -> Result<(), RnsNativeGlobalInverseProductErrorV1>;

    fn take_active_plane_opening_v1(
        &mut self,
        ordinal: usize,
        a_values: &mut [Scalar],
        u_values: &mut [Scalar],
        a_commitment_mask: &mut Scalar,
        u_commitment_mask: &mut Scalar,
    ) -> Result<(), RnsNativeGlobalInverseProductErrorV1>;

    fn take_inverse_product_mask_opening_v1(
        &mut self,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
    ) -> Result<(), RnsNativeGlobalInverseProductErrorV1>;
}

struct SecretScalarsV1(Vec<Scalar>);

impl SecretScalarsV1 {
    fn try_zeroed_v1(len: usize) -> Result<Self, RnsNativeGlobalInverseProductErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(len)
            .map_err(|_| RnsNativeGlobalInverseProductErrorV1::ResourceExhausted)?;
        values.resize(len, Scalar::zero());
        Ok(Self(values))
    }

    fn as_slice_v1(&self) -> &[Scalar] {
        &self.0
    }

    fn as_mut_slice_v1(&mut self) -> &mut [Scalar] {
        &mut self.0
    }

    fn into_vec_v1(mut self) -> Vec<Scalar> {
        core::mem::take(&mut self.0)
    }
}

impl Drop for SecretScalarsV1 {
    fn drop(&mut self) {
        let values = core::hint::black_box(&mut self.0);
        for value in values.iter_mut() {
            value.clear_secret();
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *values);
    }
}

struct SecretScalarV1(Scalar);

impl SecretScalarV1 {
    const fn zero_v1() -> Self {
        Self(Scalar::zero())
    }

    fn as_mut_v1(&mut self) -> &mut Scalar {
        &mut self.0
    }

    fn as_ref_v1(&self) -> &Scalar {
        &self.0
    }

    fn add_product_v1(&mut self, left: &Scalar, right: &Scalar) {
        self.0 += *left * *right;
    }

    fn add_assign_v1(&mut self, value: &Scalar) {
        self.0 += *value;
    }
}

impl Drop for SecretScalarV1 {
    fn drop(&mut self) {
        self.0.clear_secret();
    }
}

#[derive(Clone, Copy)]
pub(super) struct KernelContextV1 {
    pub(super) pre_z_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) post_z_transcript_digest: [u8; DIGEST_BYTES_V1],
    pub(super) z: Scalar,
}

#[derive(Clone, Copy)]
pub(super) struct KernelCommitmentsV1<'a> {
    pub(super) a: &'a [Point],
    pub(super) u: &'a [Point],
    pub(super) inverse_product_mask: Point,
}

/// Deterministic linear image of the exact authenticated active-U commitment
/// order.  The direct membership child binds this point and proves knowledge
/// of its coordinatewise-sum opening; no virtual padding plane contributes.
fn derive_active_u_sum_commitment_v1(
    geometry: KernelGeometryV1,
    active_u: &[Point],
) -> Result<Point, RnsNativeGlobalInverseProductErrorV1> {
    geometry.validate_v1()?;
    if active_u.len() != geometry.active_planes {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    let mut sum = Point::identity();
    for point in active_u.iter().copied() {
        if point.is_identity() {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidPoint);
        }
        sum += point;
    }
    if sum.is_identity() {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidPoint);
    }
    Ok(sum)
}

/// Move-only, zeroizing opening of the deterministic active-U group sum.
/// The sole destructive active-plane pass constructs this value; membership
/// consumes it exactly once and no virtual padding opening is admitted.
#[allow(
    missing_copy_implementations,
    dead_code,
    reason = "the aggregate opening and its mask must move into membership exactly once"
)]
pub(super) struct ActiveUSumOpeningV1 {
    values: SecretScalarsV1,
    mask: SecretScalarV1,
}

impl ActiveUSumOpeningV1 {
    #[cfg(test)]
    pub(super) fn from_test_values_v1(values: Vec<Scalar>, mask: Scalar) -> Self {
        Self {
            values: SecretScalarsV1(values),
            mask: SecretScalarV1(mask),
        }
    }

    pub(super) fn take_into_v1(
        mut self,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
    ) -> Result<(), RnsNativeGlobalInverseProductErrorV1> {
        if values.len() != self.values.as_slice_v1().len()
            || values.iter().copied().any(|value| !value.is_zero())
            || !commitment_mask.is_zero()
        {
            return Err(RnsNativeGlobalInverseProductErrorV1::SourceUnavailable);
        }
        for (destination, source) in values.iter_mut().zip(self.values.as_mut_slice_v1()) {
            core::mem::swap(destination, source);
        }
        core::mem::swap(commitment_mask, self.mask.as_mut_v1());
        Ok(())
    }
}

/// Transcript-complete inverse core awaiting its nonempty child frame.  None
/// of these core digests depends on the child, so membership can bind them
/// before this envelope is sealed without a Fiat-Shamir cycle.
#[allow(
    missing_copy_implementations,
    dead_code,
    reason = "the transcript-complete core must be sealed around exactly one child frame"
)]
pub(super) struct PendingInverseCoreV1 {
    geometry: KernelGeometryV1,
    messages: Vec<u8>,
    endpoint_core: Vec<u8>,
    u_sum_commitment: Point,
    rho_challenge_digest: [u8; DIGEST_BYTES_V1],
    sumcheck_transcript_digest: [u8; DIGEST_BYTES_V1],
    endpoint_transcript_digest: [u8; DIGEST_BYTES_V1],
}

impl PendingInverseCoreV1 {
    pub(super) const fn u_sum_commitment_v1(&self) -> Point {
        self.u_sum_commitment
    }

    pub(super) const fn rho_challenge_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.rho_challenge_digest
    }

    pub(super) const fn sumcheck_transcript_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.sumcheck_transcript_digest
    }

    pub(super) const fn endpoint_transcript_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.endpoint_transcript_digest
    }

    pub(super) fn seal_v1(
        self,
        downstream_residual: &[u8],
    ) -> Result<Vec<u8>, RnsNativeGlobalInverseProductErrorV1> {
        encode_wire_v1(
            self.geometry,
            &self.messages,
            &self.endpoint_core,
            downstream_residual,
        )
    }
}

/// Move-only pending inverse proof plus the only admissible active-U sum
/// opening handoff.
#[allow(
    missing_copy_implementations,
    dead_code,
    reason = "the pending core and secret aggregate opening advance together exactly once"
)]
pub(super) struct PendingInverseKernelV1 {
    core: PendingInverseCoreV1,
    u_sum_opening: ActiveUSumOpeningV1,
}

impl PendingInverseKernelV1 {
    pub(super) fn into_parts_v1(self) -> (PendingInverseCoreV1, ActiveUSumOpeningV1) {
        (self.core, self.u_sum_opening)
    }

    fn seal_v1(
        self,
        downstream_residual: &[u8],
    ) -> Result<Vec<u8>, RnsNativeGlobalInverseProductErrorV1> {
        self.core.seal_v1(downstream_residual)
    }
}

fn encode_point_v1(
    point: Point,
) -> Result<[u8; POINT_BYTES_V1], RnsNativeGlobalInverseProductErrorV1> {
    let mut encoded = [0_u8; POINT_BYTES_V1];
    point
        .write_non_identity_wire_bytes_ref(&mut encoded)
        .map_err(|_| RnsNativeGlobalInverseProductErrorV1::InvalidPoint)?;
    Ok(encoded)
}

fn manifest_digest_v1(
    geometry: KernelGeometryV1,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeGlobalInverseProductErrorV1> {
    geometry.validate_v1()?;
    let mut hash = Keccak256::new();
    hash.update(MANIFEST_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    for value in [
        geometry.active_planes,
        geometry.padded_planes,
        geometry.coordinates,
        geometry.coordinate_bits,
        geometry.plane_bits,
        geometry.rounds_v1(),
        geometry.mask_scalars_v1()?,
        SUMCHECK_DEGREE_V1,
        ENDPOINT_VECTOR_COMMITMENTS_V1,
        ENDPOINT_GATES_V1,
        ENDPOINT_CONSTRAINTS_V1,
    ] {
        hash.update(&(value as u32).to_be_bytes());
    }
    hash.update(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    hash.update(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);
    for language in [
        GEOMETRY_LANGUAGE_V1,
        ROLE_ORDER_LANGUAGE_V1,
        STREAMING_LANGUAGE_V1,
        RELATION_LANGUAGE_V1,
        MASK_LANGUAGE_V1,
        SOUNDNESS_LANGUAGE_V1,
        TRANSCRIPT_LANGUAGE_V1,
    ] {
        hash.update(&(language.len() as u16).to_be_bytes());
        hash.update(language);
    }
    let digest = hash.finalize();
    (digest != [0; DIGEST_BYTES_V1])
        .then_some(digest)
        .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidIntegrity)
}

fn commitment_set_digest_v1(
    geometry: KernelGeometryV1,
    commitments: KernelCommitmentsV1<'_>,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeGlobalInverseProductErrorV1> {
    geometry.validate_v1()?;
    if commitments.a.len() != geometry.active_planes
        || commitments.u.len() != geometry.active_planes
        || commitments.inverse_product_mask.is_identity()
    {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    let mut hash = Keccak256::new();
    hash.update(COMMITMENT_SET_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&manifest_digest_v1(geometry)?);
    hash.update(&(geometry.active_planes as u32).to_be_bytes());
    for (role, points) in [(1_u8, commitments.a), (2_u8, commitments.u)] {
        hash.update(&[role]);
        for (ordinal, point) in points.iter().copied().enumerate() {
            hash.update(&(ordinal as u32).to_be_bytes());
            hash.update(&encode_point_v1(point)?);
        }
    }
    hash.update(&[3]);
    hash.update(&encode_point_v1(commitments.inverse_product_mask)?);
    let digest = hash.finalize();
    (digest != [0; DIGEST_BYTES_V1])
        .then_some(digest)
        .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidIntegrity)
}

fn hash_v1(bytes: &[u8]) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(bytes);
    hash.finalize()
}

fn rho_challenge_digest_v1(rho: Scalar) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(RHO_DIGEST_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&rho.to_le_bytes());
    hash.finalize()
}

fn append_frame_v1(
    state: &mut Vec<u8>,
    label: &[u8],
    value: &[u8],
) -> Result<(), RnsNativeGlobalInverseProductErrorV1> {
    state.extend_from_slice(
        &u16::try_from(label.len())
            .map_err(|_| RnsNativeGlobalInverseProductErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    state.extend_from_slice(label);
    state.extend_from_slice(
        &u32::try_from(value.len())
            .map_err(|_| RnsNativeGlobalInverseProductErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    state.extend_from_slice(value);
    Ok(())
}

fn derive_nonzero_challenge_v1(
    state: &mut Vec<u8>,
    ordinal: u32,
) -> Result<Scalar, RnsNativeGlobalInverseProductErrorV1> {
    for attempt in 0..MAX_CHALLENGE_ATTEMPTS_V1 {
        let mut wide = [0_u8; 64];
        for branch in 0_u8..=1 {
            let mut input = Vec::with_capacity(
                CHALLENGE_DOMAIN_V1.len() + state.len() + core::mem::size_of::<u32>() + 2,
            );
            input.extend_from_slice(CHALLENGE_DOMAIN_V1);
            input.extend_from_slice(state);
            input.extend_from_slice(&ordinal.to_be_bytes());
            input.extend_from_slice(&[attempt, branch]);
            let start = usize::from(branch) * DIGEST_BYTES_V1;
            wide[start..start + DIGEST_BYTES_V1].copy_from_slice(&hash_v1(&input));
        }
        let challenge = Scalar::from_uniform_le_bytes(wide);
        wide.fill(0);
        if !challenge.is_zero() {
            state.push(2);
            state.extend_from_slice(&ordinal.to_be_bytes());
            state.push(attempt);
            state.extend_from_slice(&challenge.to_le_bytes());
            return Ok(challenge);
        }
    }
    Err(RnsNativeGlobalInverseProductErrorV1::ChallengeExhausted)
}

fn initial_sumcheck_transcript_v1(
    geometry: KernelGeometryV1,
    context: KernelContextV1,
    commitments: KernelCommitmentsV1<'_>,
) -> Result<(Vec<u8>, Scalar), RnsNativeGlobalInverseProductErrorV1> {
    geometry.validate_v1()?;
    if [
        context.pre_z_binding_digest,
        context.post_z_transcript_digest,
    ]
    .contains(&[0; DIGEST_BYTES_V1])
        || context.z.is_zero()
    {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidContext);
    }
    let manifest = manifest_digest_v1(geometry)?;
    let commitment_set = commitment_set_digest_v1(geometry, commitments)?;
    let mut rho_state = Vec::with_capacity(384);
    append_frame_v1(&mut rho_state, b"domain", RHO_TRANSCRIPT_DOMAIN_V1)?;
    append_frame_v1(&mut rho_state, b"manifest", &manifest)?;
    append_frame_v1(
        &mut rho_state,
        b"pre-z-binding",
        &context.pre_z_binding_digest,
    )?;
    append_frame_v1(
        &mut rho_state,
        b"post-z-transcript",
        &context.post_z_transcript_digest,
    )?;
    append_frame_v1(&mut rho_state, b"commitment-set", &commitment_set)?;
    append_frame_v1(&mut rho_state, b"z", &context.z.to_le_bytes())?;
    let rho = derive_nonzero_challenge_v1(&mut rho_state, RHO_CHALLENGE_ORDINAL_V1)?;
    let rho_digest = hash_v1(&rho_state);
    let mut state = Vec::with_capacity(4_096);
    append_frame_v1(&mut state, b"domain", SUMCHECK_TRANSCRIPT_DOMAIN_V1)?;
    append_frame_v1(&mut state, b"manifest", &manifest)?;
    append_frame_v1(&mut state, b"rho-state", &rho_digest)?;
    append_frame_v1(&mut state, b"rho", &rho.to_le_bytes())?;
    Ok((state, rho))
}

// Soundness reduction.  For fixed binding openings, a false inverse table
// gives the nonzero polynomial P(X)=sum_i e_i X^i of degree at most 2^29-1,
// so fresh nonzero rho makes P(rho)=0 with probability at most
// (2^29-1)/(pT-1).  Conditional on P(rho) != 0, the 29-round protocol is the
// ordinary individual-degree-three sum-check and contributes at most
// 29*3/(pT-1).  Fiat--Shamir applies the standard Keccak-ROM query loss; the
// manifest records the separate 512-bit reduction/conditioning term.

fn absorb_sumcheck_message_v1(
    state: &mut Vec<u8>,
    round: usize,
    compressed: [Scalar; MESSAGE_SCALARS_PER_ROUND_V1],
) -> Result<Scalar, RnsNativeGlobalInverseProductErrorV1> {
    let mut encoded = [0_u8; MESSAGE_SCALARS_PER_ROUND_V1 * SCALAR_BYTES_V1];
    for (chunk, scalar) in encoded.chunks_exact_mut(SCALAR_BYTES_V1).zip(compressed) {
        chunk.copy_from_slice(&scalar.to_le_bytes());
    }
    append_frame_v1(
        state,
        &u16::try_from(round)
            .map_err(|_| RnsNativeGlobalInverseProductErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
        &encoded,
    )?;
    derive_nonzero_challenge_v1(
        state,
        u32::try_from(round + 1)
            .map_err(|_| RnsNativeGlobalInverseProductErrorV1::ArithmeticOverflow)?,
    )
}

fn decode_scalar_v1(bytes: &[u8]) -> Result<Scalar, RnsNativeGlobalInverseProductErrorV1> {
    let encoded: [u8; SCALAR_BYTES_V1] = bytes
        .try_into()
        .map_err(|_| RnsNativeGlobalInverseProductErrorV1::InvalidScalar)?;
    Scalar::from_le_bytes_exact(encoded)
        .map_err(|_| RnsNativeGlobalInverseProductErrorV1::InvalidScalar)
}

fn evaluate_cubic_v1(coefficients: [Scalar; 4], point: Scalar) -> Scalar {
    ((coefficients[3] * point + coefficients[2]) * point + coefficients[1]) * point
        + coefficients[0]
}

fn scalar_pow_u64_v1(mut base: Scalar, mut exponent: u64) -> Scalar {
    let mut value = Scalar::one();
    while exponent != 0 {
        if exponent & 1 == 1 {
            value *= base;
        }
        base = base.square();
        exponent >>= 1;
    }
    value
}

fn interpolate_cubic_v1(
    evaluations: [Scalar; 4],
) -> Result<[Scalar; 4], RnsNativeGlobalInverseProductErrorV1> {
    let two_inverse = Scalar::from_u64(2)
        .invert()
        .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidContext)?;
    let six_inverse = Scalar::from_u64(6)
        .invert()
        .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidContext)?;
    let d1 = evaluations[1] - evaluations[0];
    let d2 = evaluations[2] - evaluations[0];
    let d3 = evaluations[3] - evaluations[0];
    let cubic = (d3 - Scalar::from_u64(3) * d2 + Scalar::from_u64(3) * d1) * six_inverse;
    let quadratic = (d2 - Scalar::from_u64(2) * d1 - Scalar::from_u64(6) * cubic) * two_inverse;
    let linear = d1 - quadratic - cubic;
    Ok([evaluations[0], linear, quadratic, cubic])
}

fn reconstruct_cubic_v1(
    claim: Scalar,
    compressed: [Scalar; MESSAGE_SCALARS_PER_ROUND_V1],
) -> [Scalar; 4] {
    let [constant, quadratic, cubic] = compressed;
    let linear = claim - Scalar::from_u64(2) * constant - quadratic - cubic;
    [constant, linear, quadratic, cubic]
}

fn fold_prefix_v1(
    values: &mut [Scalar],
    challenges: &[Scalar],
) -> Result<usize, RnsNativeGlobalInverseProductErrorV1> {
    if values.is_empty() || !values.len().is_power_of_two() {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    let mut live = values.len();
    for challenge in challenges.iter().copied() {
        if live < 2 {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
        }
        for index in 0..live / 2 {
            let low = values[2 * index];
            let high = values[2 * index + 1];
            values[index] = low + challenge * (high - low);
        }
        for value in &mut values[live / 2..live] {
            value.clear_secret();
        }
        live /= 2;
    }
    Ok(live)
}

fn fold_one_round_v1(
    values: &mut [Scalar],
    live: usize,
    challenge: Scalar,
) -> Result<usize, RnsNativeGlobalInverseProductErrorV1> {
    if live < 2 || live > values.len() || !live.is_power_of_two() {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    for index in 0..live / 2 {
        let low = values[2 * index];
        let high = values[2 * index + 1];
        values[index] = low + challenge * (high - low);
    }
    for value in &mut values[live / 2..live] {
        value.clear_secret();
    }
    Ok(live / 2)
}

fn eq_weights_v1(point: &[Scalar]) -> Result<Vec<Scalar>, RnsNativeGlobalInverseProductErrorV1> {
    let size = 1_usize
        .checked_shl(
            u32::try_from(point.len())
                .map_err(|_| RnsNativeGlobalInverseProductErrorV1::ArithmeticOverflow)?,
        )
        .ok_or(RnsNativeGlobalInverseProductErrorV1::ArithmeticOverflow)?;
    let mut weights = Vec::new();
    weights
        .try_reserve_exact(size)
        .map_err(|_| RnsNativeGlobalInverseProductErrorV1::ResourceExhausted)?;
    weights.push(Scalar::one());
    for challenge in point.iter().copied() {
        let populated = weights.len();
        for index in 0..populated {
            let low = weights[index] * (Scalar::one() - challenge);
            let high = weights[index] * challenge;
            weights[index] = low;
            weights.push(high);
        }
        // Existing lower-bit ordinals remain contiguous in each half:
        // `[old*(1-r_j), old*r_j]`.  This is the little-endian table order
        // consumed by adjacent-pair folding of bit zero, then bit one, etc.
    }
    if weights.len() != size {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    Ok(weights)
}

fn multilinear_evaluate_v1(
    values: &[Scalar],
    point: &[Scalar],
) -> Result<Scalar, RnsNativeGlobalInverseProductErrorV1> {
    let weights = eq_weights_v1(point)?;
    if values.len() != weights.len() {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    Ok(values
        .iter()
        .copied()
        .zip(weights)
        .fold(Scalar::zero(), |sum, (value, weight)| sum + value * weight))
}

fn rho_endpoint_evaluation_v1(
    geometry: KernelGeometryV1,
    rho: Scalar,
    point: &[Scalar],
) -> Result<Scalar, RnsNativeGlobalInverseProductErrorV1> {
    geometry.validate_v1()?;
    if point.len() != geometry.rounds_v1() {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    let mut power = rho;
    let mut evaluation = Scalar::one();
    for challenge in &point[..geometry.coordinate_bits] {
        evaluation *= Scalar::one() - *challenge + *challenge * power;
        power = power.square();
    }
    // `power` is now rho^coordinates.  Continuing the repeated squaring
    // exactly matches exponents coordinates*2^j on the plane bits.
    for challenge in &point[geometry.coordinate_bits..] {
        evaluation *= Scalar::one() - *challenge + *challenge * power;
        power = power.square();
    }
    Ok(evaluation)
}

fn mask_terminal_weights_v1(
    challenges: &[Scalar],
) -> Result<Vec<Scalar>, RnsNativeGlobalInverseProductErrorV1> {
    if challenges.is_empty() {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    let half = Scalar::from_u64(2)
        .invert()
        .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidContext)?;
    let mut future_scale = Scalar::one();
    let mut reversed = Vec::new();
    reversed
        .try_reserve_exact(challenges.len() * MASK_SCALARS_PER_ROUND_V1)
        .map_err(|_| RnsNativeGlobalInverseProductErrorV1::ResourceExhausted)?;
    for challenge in challenges.iter().rev().copied() {
        reversed.extend_from_slice(&[
            future_scale * (challenge - half),
            future_scale * (challenge.square() - half),
            future_scale * (challenge.square() * challenge - half),
        ]);
        future_scale *= half;
    }
    let mut weights = Vec::with_capacity(reversed.len());
    for chunk in reversed.chunks_exact(MASK_SCALARS_PER_ROUND_V1).rev() {
        weights.extend_from_slice(chunk);
    }
    Ok(weights)
}

fn apply_round_mask_v1(
    raw: [Scalar; 4],
    previous_carry: Scalar,
    mask: [Scalar; MASK_SCALARS_PER_ROUND_V1],
) -> Result<[Scalar; 4], RnsNativeGlobalInverseProductErrorV1> {
    let half = Scalar::from_u64(2)
        .invert()
        .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidContext)?;
    let [linear, quadratic, cubic] = mask;
    let constant = half * previous_carry - half * (linear + quadratic + cubic);
    Ok([
        raw[0] + constant,
        raw[1] + linear,
        raw[2] + quadratic,
        raw[3] + cubic,
    ])
}

// For every round q_j(0)+q_j(1)=carry_{j-1}, while
// carry_j=q_j(r_j).  Consequently subtracting q_j from any accepted masked
// transcript recovers an ordinary sum-check transcript, and the endpoint
// mask functional is exactly carry_28.  Fresh (m1,m2,m3) map bijectively to
// the transmitted (constant,quadratic,cubic) coefficients, giving perfect
// message hiding before the computationally-ZK endpoint argument.

fn coordinate_round_evaluations_v1<P: RnsNativeGlobalInverseProductOpeningSourceV1>(
    geometry: KernelGeometryV1,
    source: &mut P,
    z: Scalar,
    rho: Scalar,
    prefix: &[Scalar],
    a_buffer: &mut SecretScalarsV1,
    u_buffer: &mut SecretScalarsV1,
    rho_coordinates: &[Scalar],
) -> Result<[Scalar; 4], RnsNativeGlobalInverseProductErrorV1> {
    if prefix.len() >= geometry.coordinate_bits
        || a_buffer.as_slice_v1().len() != geometry.coordinates
        || u_buffer.as_slice_v1().len() != geometry.coordinates
        || rho_coordinates.len() != geometry.coordinates
    {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    let mut rho_folded = rho_coordinates.to_vec();
    let live = fold_prefix_v1(&mut rho_folded, prefix)?;
    let mut evaluations = [Scalar::zero(); 4];
    let mut plane_weight = Scalar::one();
    let plane_ratio = scalar_pow_u64_v1(rho, geometry.coordinates as u64);
    for plane in 0..geometry.active_planes {
        a_buffer.as_mut_slice_v1()[..live].fill(Scalar::zero());
        u_buffer.as_mut_slice_v1()[..live].fill(Scalar::zero());
        source.replay_active_plane_values_v1(
            plane,
            prefix,
            &mut a_buffer.as_mut_slice_v1()[..live],
            &mut u_buffer.as_mut_slice_v1()[..live],
        )?;
        if live < 2 {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
        }
        for index in 0..live / 2 {
            let a0 = a_buffer.as_slice_v1()[2 * index];
            let da = a_buffer.as_slice_v1()[2 * index + 1] - a0;
            let u0 = u_buffer.as_slice_v1()[2 * index];
            let du = u_buffer.as_slice_v1()[2 * index + 1] - u0;
            let r0 = rho_folded[2 * index];
            let dr = rho_folded[2 * index + 1] - r0;
            for (evaluation, point) in evaluations.iter_mut().zip([
                Scalar::zero(),
                Scalar::one(),
                Scalar::from_u64(2),
                Scalar::from_u64(3),
            ]) {
                let a = a0 + point * da;
                let u = u0 + point * du;
                let r = plane_weight * (r0 + point * dr);
                *evaluation += r * ((z - a) * u - Scalar::one());
            }
        }
        plane_weight *= plane_ratio;
    }
    Ok(evaluations)
}

fn materialize_plane_endpoint_tables_v1<P: RnsNativeGlobalInverseProductOpeningSourceV1>(
    geometry: KernelGeometryV1,
    source: &mut P,
    z_inverse: Scalar,
    coordinate_point: &[Scalar],
    a_buffer: &mut SecretScalarsV1,
    u_buffer: &mut SecretScalarsV1,
) -> Result<(SecretScalarsV1, SecretScalarsV1), RnsNativeGlobalInverseProductErrorV1> {
    if coordinate_point.len() != geometry.coordinate_bits {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    let mut a_planes = SecretScalarsV1::try_zeroed_v1(geometry.padded_planes)?;
    let mut u_planes = SecretScalarsV1::try_zeroed_v1(geometry.padded_planes)?;
    for plane in 0..geometry.active_planes {
        a_buffer.as_mut_slice_v1()[..1].fill(Scalar::zero());
        u_buffer.as_mut_slice_v1()[..1].fill(Scalar::zero());
        source.replay_active_plane_values_v1(
            plane,
            coordinate_point,
            &mut a_buffer.as_mut_slice_v1()[..1],
            &mut u_buffer.as_mut_slice_v1()[..1],
        )?;
        a_planes.as_mut_slice_v1()[plane] = a_buffer.as_slice_v1()[0];
        u_planes.as_mut_slice_v1()[plane] = u_buffer.as_slice_v1()[0];
    }
    u_planes.as_mut_slice_v1()[geometry.active_planes..].fill(z_inverse);
    Ok((a_planes, u_planes))
}

fn plane_round_evaluations_v1(
    z: Scalar,
    rho_coordinate_evaluation: Scalar,
    a_planes: &[Scalar],
    u_planes: &[Scalar],
    rho_planes: &[Scalar],
    live: usize,
) -> Result<[Scalar; 4], RnsNativeGlobalInverseProductErrorV1> {
    if live < 2
        || !live.is_power_of_two()
        || a_planes.len() < live
        || u_planes.len() < live
        || rho_planes.len() < live
    {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    let mut evaluations = [Scalar::zero(); 4];
    for index in 0..live / 2 {
        let a0 = a_planes[2 * index];
        let da = a_planes[2 * index + 1] - a0;
        let u0 = u_planes[2 * index];
        let du = u_planes[2 * index + 1] - u0;
        let r0 = rho_planes[2 * index];
        let dr = rho_planes[2 * index + 1] - r0;
        for (evaluation, point) in evaluations.iter_mut().zip([
            Scalar::zero(),
            Scalar::one(),
            Scalar::from_u64(2),
            Scalar::from_u64(3),
        ]) {
            let a = a0 + point * da;
            let u = u0 + point * du;
            let r = rho_coordinate_evaluation * (r0 + point * dr);
            *evaluation += r * ((z - a) * u - Scalar::one());
        }
    }
    Ok(evaluations)
}

fn fold_commitment_v1<S: ProofSuite<Scalar = Scalar, Point = Point>>(
    points: &[Point],
    weights: &[Scalar],
) -> Result<Point, RnsNativeGlobalInverseProductErrorV1> {
    if points.len() != weights.len() || points.is_empty() {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    let terms: Vec<(Scalar, Point)> = weights
        .iter()
        .copied()
        .zip(points.iter().copied())
        .collect();
    let point = multiexp::<S>(&terms);
    if point.is_identity() {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidPoint);
    }
    Ok(point)
}

fn public_constant_vector_commitment_v1<S: ProofSuite<Scalar = Scalar, Point = Point>>(
    coordinates: usize,
    value: Scalar,
) -> Result<Point, RnsNativeGlobalInverseProductErrorV1> {
    let generators = S::generators().reduce(coordinates)?;
    let terms: Vec<(Scalar, Point)> = generators
        .g_bold
        .iter()
        .copied()
        .map(|generator| (value, generator))
        .collect();
    let point = multiexp::<S>(&terms);
    if point.is_identity() {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidPoint);
    }
    Ok(point)
}

fn folded_endpoint_commitments_v1<S: ProofSuite<Scalar = Scalar, Point = Point>>(
    geometry: KernelGeometryV1,
    commitments: KernelCommitmentsV1<'_>,
    z_inverse: Scalar,
    plane_point: &[Scalar],
) -> Result<(Point, Point, Vec<Scalar>), RnsNativeGlobalInverseProductErrorV1> {
    let weights = eq_weights_v1(plane_point)?;
    if weights.len() != geometry.padded_planes
        || commitments.a.len() != geometry.active_planes
        || commitments.u.len() != geometry.active_planes
    {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    let active_weights = &weights[..geometry.active_planes];
    let a = fold_commitment_v1::<S>(commitments.a, active_weights)?;
    let mut u = fold_commitment_v1::<S>(commitments.u, active_weights)?;
    let padding_weight = weights[geometry.active_planes..]
        .iter()
        .copied()
        .fold(Scalar::zero(), |sum, weight| sum + weight);
    if !padding_weight.is_zero() {
        let padding = public_constant_vector_commitment_v1::<S>(geometry.coordinates, z_inverse)?;
        u = u + padding.mul_scalar(padding_weight);
        if u.is_identity() {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidPoint);
        }
    }
    Ok((a, u, weights))
}

/// Bind the sum-check endpoint with one multiplication gate and no separate
/// scalar commitments.  With `A_r`, `U_r`, and `H_r` denoting linear
/// functionals of the three vector openings, the constraints are exactly
/// `aL + A_r - z = 0`, `aR - U_r = 0`, and
/// `R_r*aO + H_r - R_r - final_claim = 0`; the generalized-Bulletproof
/// circuit itself enforces `aO = aL*aR`.
fn build_endpoint_statement_v1<S: ProofSuite<Scalar = Scalar, Point = Point>>(
    geometry: KernelGeometryV1,
    a_commitment: Point,
    u_commitment: Point,
    inverse_product_mask_commitment: Point,
    coordinate_point: &[Scalar],
    sumcheck_challenges: &[Scalar],
    rho_evaluation: Scalar,
    final_claim: Scalar,
    z: Scalar,
) -> Result<ArithmeticCircuitStatement<'static, S>, RnsNativeGlobalInverseProductErrorV1> {
    let coordinate_weights = eq_weights_v1(coordinate_point)?;
    let mask_weights = mask_terminal_weights_v1(sumcheck_challenges)?;
    if coordinate_weights.len() != geometry.coordinates
        || mask_weights.len() != geometry.mask_scalars_v1()?
    {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    let mut bind_a = LinComb::empty()
        .term(Scalar::one(), Variable::aL(0))
        .constant(-z);
    let mut bind_u = LinComb::empty().term(Scalar::one(), Variable::aR(0));
    for (index, weight) in coordinate_weights.into_iter().enumerate() {
        bind_a = bind_a.term(
            weight,
            Variable::CG {
                commitment: 0,
                index,
            },
        );
        bind_u = bind_u.term(
            -weight,
            Variable::CG {
                commitment: 1,
                index,
            },
        );
    }
    let mut endpoint = LinComb::empty()
        .term(rho_evaluation, Variable::aO(0))
        .constant(-rho_evaluation - final_claim);
    for (index, weight) in mask_weights.into_iter().enumerate() {
        endpoint = endpoint.term(
            weight,
            Variable::CG {
                commitment: 2,
                index,
            },
        );
    }
    Ok(ArithmeticCircuitStatement::new(
        S::generators().reduce(geometry.coordinates)?,
        vec![bind_a, bind_u, endpoint],
        vec![a_commitment, u_commitment, inverse_product_mask_commitment],
        Vec::new(),
    )?)
}

fn endpoint_initial_state_v1(
    geometry: KernelGeometryV1,
    sumcheck_state: &[u8],
    commitments: [Point; ENDPOINT_VECTOR_COMMITMENTS_V1],
    final_claim: Scalar,
) -> Result<Vec<u8>, RnsNativeGlobalInverseProductErrorV1> {
    let mut state = Vec::with_capacity(512);
    append_frame_v1(&mut state, b"domain", ENDPOINT_TRANSCRIPT_DOMAIN_V1)?;
    append_frame_v1(&mut state, b"manifest", &manifest_digest_v1(geometry)?)?;
    append_frame_v1(&mut state, b"sumcheck-state", &hash_v1(sumcheck_state))?;
    append_frame_v1(&mut state, b"final-claim", &final_claim.to_le_bytes())?;
    for (ordinal, point) in commitments.into_iter().enumerate() {
        append_frame_v1(
            &mut state,
            &u16::try_from(ordinal)
                .map_err(|_| RnsNativeGlobalInverseProductErrorV1::ArithmeticOverflow)?
                .to_be_bytes(),
            &encode_point_v1(point)?,
        )?;
    }
    Ok(state)
}

struct EndpointProverTranscriptV1<S: ProofSuite<Scalar = Scalar, Point = Point>> {
    state: Vec<u8>,
    proof: Vec<u8>,
    challenge_ordinal: u32,
    expected_bytes: usize,
    suite: PhantomData<S>,
}

impl<S: ProofSuite<Scalar = Scalar, Point = Point>> EndpointProverTranscriptV1<S> {
    fn new_v1(state: Vec<u8>, expected_bytes: usize) -> Self {
        Self {
            state,
            proof: Vec::with_capacity(expected_bytes),
            challenge_ordinal: 0,
            expected_bytes,
            suite: PhantomData,
        }
    }

    fn finish_v1(
        self,
    ) -> Result<(Vec<u8>, [u8; DIGEST_BYTES_V1]), RnsNativeGlobalInverseProductErrorV1> {
        if self.proof.len() != self.expected_bytes || self.proof.capacity() != self.expected_bytes {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidEndpoint);
        }
        Ok((self.proof, hash_v1(&self.state)))
    }
}

impl<S: ProofSuite<Scalar = Scalar, Point = Point>> ProverTranscript<S>
    for EndpointProverTranscriptV1<S>
{
    fn push_scalar(&mut self, scalar: &Scalar) -> Result<(), GeneralizedBulletproofErrorV1> {
        let encoded = scalar.to_le_bytes();
        self.proof.extend_from_slice(&encoded);
        self.state.push(0);
        self.state.extend_from_slice(&encoded);
        if self.proof.len() > self.expected_bytes {
            return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);
        }
        Ok(())
    }

    fn push_point(&mut self, point: &Point) -> Result<(), GeneralizedBulletproofErrorV1> {
        let mut encoded = [0_u8; POINT_BYTES_V1];
        point
            .write_non_identity_wire_bytes_ref(&mut encoded)
            .map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?;
        self.proof.extend_from_slice(&encoded);
        self.state.push(1);
        self.state.extend_from_slice(&encoded);
        if self.proof.len() > self.expected_bytes {
            return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);
        }
        Ok(())
    }

    fn challenge(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        let challenge = derive_nonzero_challenge_v1(&mut self.state, self.challenge_ordinal)
            .map_err(|_| GeneralizedBulletproofErrorV1::TranscriptChallengeExhausted)?;
        self.challenge_ordinal = self
            .challenge_ordinal
            .checked_add(1)
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        Ok(challenge)
    }
}

struct EndpointVerifierTranscriptV1<'a, S: ProofSuite<Scalar = Scalar, Point = Point>> {
    state: Vec<u8>,
    proof: &'a [u8],
    cursor: usize,
    challenge_ordinal: u32,
    suite: PhantomData<S>,
}

impl<'a, S: ProofSuite<Scalar = Scalar, Point = Point>> EndpointVerifierTranscriptV1<'a, S> {
    fn new_v1(
        state: Vec<u8>,
        proof: &'a [u8],
        expected_bytes: usize,
    ) -> Result<Self, RnsNativeGlobalInverseProductErrorV1> {
        if proof.len() != expected_bytes {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidEndpoint);
        }
        Ok(Self {
            state,
            proof,
            cursor: 0,
            challenge_ordinal: 0,
            suite: PhantomData,
        })
    }

    fn take_v1(&mut self, count: usize) -> Result<&'a [u8], GeneralizedBulletproofErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let value =
            self.proof
                .get(self.cursor..end)
                .ok_or(GeneralizedBulletproofErrorV1::ProofLength {
                    actual: self.proof.len(),
                    expected: end,
                })?;
        self.cursor = end;
        Ok(value)
    }

    fn finish_v1(self) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeGlobalInverseProductErrorV1> {
        if self.cursor != self.proof.len() {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidEndpoint);
        }
        Ok(hash_v1(&self.state))
    }
}

impl<S: ProofSuite<Scalar = Scalar, Point = Point>> VerifierTranscript<S>
    for EndpointVerifierTranscriptV1<'_, S>
{
    fn read_scalar(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        let encoded: [u8; SCALAR_BYTES_V1] = self
            .take_v1(SCALAR_BYTES_V1)?
            .try_into()
            .map_err(|_| GeneralizedBulletproofErrorV1::ScalarEncoding)?;
        let scalar = Scalar::from_le_bytes_exact(encoded)
            .map_err(|_| GeneralizedBulletproofErrorV1::ScalarEncoding)?;
        self.state.push(0);
        self.state.extend_from_slice(&encoded);
        Ok(scalar)
    }

    fn read_point(&mut self) -> Result<Point, GeneralizedBulletproofErrorV1> {
        let encoded: [u8; POINT_BYTES_V1] = self
            .take_v1(POINT_BYTES_V1)?
            .try_into()
            .map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?;
        let point = Point::from_non_identity_wire_bytes_exact(&encoded)
            .map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?;
        self.state.push(1);
        self.state.extend_from_slice(&encoded);
        Ok(point)
    }

    fn challenge(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        let challenge = derive_nonzero_challenge_v1(&mut self.state, self.challenge_ordinal)
            .map_err(|_| GeneralizedBulletproofErrorV1::TranscriptChallengeExhausted)?;
        self.challenge_ordinal = self
            .challenge_ordinal
            .checked_add(1)
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        Ok(challenge)
    }
}

// Kept separate from the transport wrapper so membership can bind the
// transcript-complete core before the inverse envelope admits its child.
#[allow(clippy::too_many_arguments)]
pub(super) fn prove_pending_kernel_for_suite_v1<S, P, R>(
    geometry: KernelGeometryV1,
    context: KernelContextV1,
    commitments: KernelCommitmentsV1<'_>,
    mut source: P,
    rng: &mut R,
) -> Result<PendingInverseKernelV1, RnsNativeGlobalInverseProductErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
    P: RnsNativeGlobalInverseProductOpeningSourceV1,
    R: ProofRandomSource,
{
    geometry.validate_v1()?;
    let u_sum_commitment = derive_active_u_sum_commitment_v1(geometry, commitments.u)?;
    let (mut sumcheck_state, rho) = initial_sumcheck_transcript_v1(geometry, context, commitments)?;
    let z_inverse = context
        .z
        .invert()
        .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidContext)?;
    let mask_len = geometry.mask_scalars_v1()?;
    let mut mask_values = SecretScalarsV1::try_zeroed_v1(mask_len)?;
    let mut inverse_product_mask_blinding = SecretScalarV1::zero_v1();
    source.take_inverse_product_mask_opening_v1(
        mask_values.as_mut_slice_v1(),
        inverse_product_mask_blinding.as_mut_v1(),
    )?;
    let mut a_buffer = SecretScalarsV1::try_zeroed_v1(geometry.coordinates)?;
    let mut u_buffer = SecretScalarsV1::try_zeroed_v1(geometry.coordinates)?;
    let mut rho_coordinates = Vec::with_capacity(geometry.coordinates);
    let mut power = Scalar::one();
    for _ in 0..geometry.coordinates {
        rho_coordinates.push(power);
        power *= rho;
    }
    let plane_ratio = power;
    let mut messages = Vec::with_capacity(geometry.rounds_v1() * 3 * SCALAR_BYTES_V1);
    let mut challenges = Vec::with_capacity(geometry.rounds_v1());
    let mut raw_claim = Scalar::zero();
    let mut masked_claim = Scalar::zero();
    let mut mask_carry = Scalar::zero();
    for round in 0..geometry.coordinate_bits {
        let evaluations = coordinate_round_evaluations_v1(
            geometry,
            &mut source,
            context.z,
            rho,
            &challenges,
            &mut a_buffer,
            &mut u_buffer,
            &rho_coordinates,
        )?;
        if evaluations[0] + evaluations[1] != raw_claim {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidSumcheck);
        }
        let raw = interpolate_cubic_v1(evaluations)?;
        let mask = [
            mask_values.as_slice_v1()[3 * round],
            mask_values.as_slice_v1()[3 * round + 1],
            mask_values.as_slice_v1()[3 * round + 2],
        ];
        let masked = apply_round_mask_v1(raw, mask_carry, mask)?;
        if masked[0] + evaluate_cubic_v1(masked, Scalar::one()) != masked_claim {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidSumcheck);
        }
        let compressed = [masked[0], masked[2], masked[3]];
        for scalar in compressed {
            messages.extend_from_slice(&scalar.to_le_bytes());
        }
        let challenge = absorb_sumcheck_message_v1(&mut sumcheck_state, round, compressed)?;
        challenges.push(challenge);
        raw_claim = evaluate_cubic_v1(raw, challenge);
        mask_carry = evaluate_cubic_v1(
            [
                masked[0] - raw[0],
                masked[1] - raw[1],
                masked[2] - raw[2],
                masked[3] - raw[3],
            ],
            challenge,
        );
        masked_claim = evaluate_cubic_v1(masked, challenge);
    }
    let coordinate_point = challenges.clone();
    let (mut a_planes, mut u_planes) = materialize_plane_endpoint_tables_v1(
        geometry,
        &mut source,
        z_inverse,
        &coordinate_point,
        &mut a_buffer,
        &mut u_buffer,
    )?;
    let rho_coordinate_evaluation = multilinear_evaluate_v1(&rho_coordinates, &coordinate_point)?;
    let mut rho_planes = Vec::with_capacity(geometry.padded_planes);
    let mut plane_power = Scalar::one();
    for _ in 0..geometry.padded_planes {
        rho_planes.push(plane_power);
        plane_power *= plane_ratio;
    }
    let mut live = geometry.padded_planes;
    for plane_round in 0..geometry.plane_bits {
        let round = geometry.coordinate_bits + plane_round;
        let evaluations = plane_round_evaluations_v1(
            context.z,
            rho_coordinate_evaluation,
            a_planes.as_slice_v1(),
            u_planes.as_slice_v1(),
            &rho_planes,
            live,
        )?;
        if evaluations[0] + evaluations[1] != raw_claim {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidSumcheck);
        }
        let raw = interpolate_cubic_v1(evaluations)?;
        let mask = [
            mask_values.as_slice_v1()[3 * round],
            mask_values.as_slice_v1()[3 * round + 1],
            mask_values.as_slice_v1()[3 * round + 2],
        ];
        let masked = apply_round_mask_v1(raw, mask_carry, mask)?;
        if masked[0] + evaluate_cubic_v1(masked, Scalar::one()) != masked_claim {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidSumcheck);
        }
        let compressed = [masked[0], masked[2], masked[3]];
        for scalar in compressed {
            messages.extend_from_slice(&scalar.to_le_bytes());
        }
        let challenge = absorb_sumcheck_message_v1(&mut sumcheck_state, round, compressed)?;
        challenges.push(challenge);
        raw_claim = evaluate_cubic_v1(raw, challenge);
        mask_carry = evaluate_cubic_v1(
            [
                masked[0] - raw[0],
                masked[1] - raw[1],
                masked[2] - raw[2],
                masked[3] - raw[3],
            ],
            challenge,
        );
        masked_claim = evaluate_cubic_v1(masked, challenge);
        live = fold_one_round_v1(a_planes.as_mut_slice_v1(), live, challenge)?;
        if fold_one_round_v1(u_planes.as_mut_slice_v1(), live * 2, challenge)? != live
            || fold_one_round_v1(&mut rho_planes, live * 2, challenge)? != live
        {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
        }
    }
    if live != 1 || challenges.len() != geometry.rounds_v1() {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    let plane_point = &challenges[geometry.coordinate_bits..];
    let (a_commitment, u_commitment, plane_weights) =
        folded_endpoint_commitments_v1::<S>(geometry, commitments, z_inverse, plane_point)?;
    let mut folded_a = SecretScalarsV1::try_zeroed_v1(geometry.coordinates)?;
    let mut folded_u = SecretScalarsV1::try_zeroed_v1(geometry.coordinates)?;
    let mut folded_a_mask = SecretScalarV1::zero_v1();
    let mut folded_u_mask = SecretScalarV1::zero_v1();
    let mut u_sum_values = SecretScalarsV1::try_zeroed_v1(geometry.coordinates)?;
    let mut u_sum_mask = SecretScalarV1::zero_v1();
    for plane in 0..geometry.active_planes {
        a_buffer.as_mut_slice_v1().fill(Scalar::zero());
        u_buffer.as_mut_slice_v1().fill(Scalar::zero());
        let mut a_mask = SecretScalarV1::zero_v1();
        let mut u_mask = SecretScalarV1::zero_v1();
        source.take_active_plane_opening_v1(
            plane,
            a_buffer.as_mut_slice_v1(),
            u_buffer.as_mut_slice_v1(),
            a_mask.as_mut_v1(),
            u_mask.as_mut_v1(),
        )?;
        let weight = plane_weights[plane];
        for index in 0..geometry.coordinates {
            folded_a.as_mut_slice_v1()[index] += weight * a_buffer.as_slice_v1()[index];
            folded_u.as_mut_slice_v1()[index] += weight * u_buffer.as_slice_v1()[index];
            u_sum_values.as_mut_slice_v1()[index] += u_buffer.as_slice_v1()[index];
        }
        folded_a_mask.add_product_v1(&weight, a_mask.as_ref_v1());
        folded_u_mask.add_product_v1(&weight, u_mask.as_ref_v1());
        u_sum_mask.add_assign_v1(u_mask.as_ref_v1());
    }
    let padding_weight = plane_weights[geometry.active_planes..]
        .iter()
        .copied()
        .fold(Scalar::zero(), |sum, weight| sum + weight);
    for value in folded_u.as_mut_slice_v1() {
        *value += padding_weight * z_inverse;
    }
    let a_endpoint = multilinear_evaluate_v1(folded_a.as_slice_v1(), &coordinate_point)?;
    let u_endpoint = multilinear_evaluate_v1(folded_u.as_slice_v1(), &coordinate_point)?;
    if a_endpoint != a_planes.as_slice_v1()[0]
        || u_endpoint != u_planes.as_slice_v1()[0]
        || raw_claim
            != rho_endpoint_evaluation_v1(geometry, rho, &challenges)?
                * ((context.z - a_endpoint) * u_endpoint - Scalar::one())
        || masked_claim != raw_claim + mask_carry
    {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidSumcheck);
    }
    let rho_evaluation = rho_endpoint_evaluation_v1(geometry, rho, &challenges)?;
    let endpoint_state = endpoint_initial_state_v1(
        geometry,
        &sumcheck_state,
        [a_commitment, u_commitment, commitments.inverse_product_mask],
        masked_claim,
    )?;
    let endpoint_bytes = endpoint_core_bytes_v1(geometry)?;
    let mut endpoint_transcript =
        EndpointProverTranscriptV1::<S>::new_v1(endpoint_state, endpoint_bytes);
    let a_l = context.z - a_endpoint;
    let a_r = u_endpoint;
    let witness = ArithmeticCircuitWitness::<S>::new(
        vec![a_l],
        vec![a_r],
        vec![
            VectorCommitmentOpening::take_mask_from_slot(
                folded_a.into_vec_v1(),
                folded_a_mask.as_mut_v1(),
            ),
            VectorCommitmentOpening::take_mask_from_slot(
                folded_u.into_vec_v1(),
                folded_u_mask.as_mut_v1(),
            ),
            VectorCommitmentOpening::take_mask_from_slot(
                mask_values.into_vec_v1(),
                inverse_product_mask_blinding.as_mut_v1(),
            ),
        ],
    )?;
    build_endpoint_statement_v1::<S>(
        geometry,
        a_commitment,
        u_commitment,
        commitments.inverse_product_mask,
        &coordinate_point,
        &challenges,
        rho_evaluation,
        masked_claim,
        context.z,
    )?
    .prove(rng, &mut endpoint_transcript, witness)?;
    let (endpoint_core, endpoint_transcript_digest) = endpoint_transcript.finish_v1()?;
    Ok(PendingInverseKernelV1 {
        core: PendingInverseCoreV1 {
            geometry,
            messages,
            endpoint_core,
            u_sum_commitment,
            rho_challenge_digest: rho_challenge_digest_v1(rho),
            sumcheck_transcript_digest: hash_v1(&sumcheck_state),
            endpoint_transcript_digest,
        },
        u_sum_opening: ActiveUSumOpeningV1 {
            values: u_sum_values,
            mask: u_sum_mask,
        },
    })
}

// Compatibility wrapper: its transcript and canonical wire remain byte-for-
// byte identical; an unused aggregate opening is zeroized when the pending
// owner is sealed and dropped.
#[allow(clippy::too_many_arguments)]
fn prove_kernel_for_suite_v1<S, P, R>(
    geometry: KernelGeometryV1,
    context: KernelContextV1,
    commitments: KernelCommitmentsV1<'_>,
    source: P,
    downstream_residual: &[u8],
    rng: &mut R,
) -> Result<Vec<u8>, RnsNativeGlobalInverseProductErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
    P: RnsNativeGlobalInverseProductOpeningSourceV1,
    R: ProofRandomSource,
{
    if downstream_residual.is_empty() {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    prove_pending_kernel_for_suite_v1::<S, _, _>(geometry, context, commitments, source, rng)?
        .seal_v1(downstream_residual)
}

fn endpoint_core_bytes_v1(
    geometry: KernelGeometryV1,
) -> Result<usize, RnsNativeGlobalInverseProductErrorV1> {
    geometry.validate_v1()?;
    let ni = 2 + 2 * (ENDPOINT_VECTOR_COMMITMENTS_V1 / 2);
    let l_polynomials = ni + 2;
    let t_polynomials = 2 * l_polynomials - 1;
    let fixed_points = 3 + t_polynomials - 1;
    let ipa_points = 2 * geometry.coordinate_bits;
    fixed_points
        .checked_add(ipa_points)
        .and_then(|points| points.checked_mul(POINT_BYTES_V1))
        .and_then(|bytes| bytes.checked_add(ENDPOINT_SCALARS_V1 * SCALAR_BYTES_V1))
        .ok_or(RnsNativeGlobalInverseProductErrorV1::ArithmeticOverflow)
}

fn encode_wire_v1(
    geometry: KernelGeometryV1,
    messages: &[u8],
    endpoint_core: &[u8],
    downstream_residual: &[u8],
) -> Result<Vec<u8>, RnsNativeGlobalInverseProductErrorV1> {
    geometry.validate_v1()?;
    let expected_messages = geometry
        .rounds_v1()
        .checked_mul(MESSAGE_SCALARS_PER_ROUND_V1 * SCALAR_BYTES_V1)
        .ok_or(RnsNativeGlobalInverseProductErrorV1::ArithmeticOverflow)?;
    let expected_endpoint = endpoint_core_bytes_v1(geometry)?;
    if messages.len() != expected_messages
        || endpoint_core.len() != expected_endpoint
        || downstream_residual.is_empty()
    {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    let total = HEADER_BYTES_V1
        .checked_add(messages.len())
        .and_then(|value| value.checked_add(endpoint_core.len()))
        .and_then(|value| value.checked_add(downstream_residual.len()))
        .and_then(|value| value.checked_add(CODEC_DIGEST_BYTES_V1))
        .ok_or(RnsNativeGlobalInverseProductErrorV1::ArithmeticOverflow)?;
    if geometry == KernelGeometryV1::PRODUCTION && total > PARENT_RESIDUAL_CAP_BYTES_V1 {
        return Err(RnsNativeGlobalInverseProductErrorV1::ProofCapExceeded);
    }
    let mut wire = Vec::with_capacity(total);
    wire.extend_from_slice(&MAGIC_V1);
    wire.extend_from_slice(&[VERSION_V1, FLAGS_V1]);
    wire.extend_from_slice(&(HEADER_BYTES_V1 as u16).to_be_bytes());
    wire.extend_from_slice(
        &u32::try_from(total)
            .map_err(|_| RnsNativeGlobalInverseProductErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    wire.extend_from_slice(&(geometry.active_planes as u16).to_be_bytes());
    wire.extend_from_slice(&(geometry.padded_planes as u16).to_be_bytes());
    wire.extend_from_slice(&(geometry.coordinates as u16).to_be_bytes());
    wire.extend_from_slice(&[geometry.coordinate_bits as u8, geometry.plane_bits as u8]);
    wire.extend_from_slice(&[geometry.rounds_v1() as u8, SUMCHECK_DEGREE_V1 as u8]);
    wire.extend_from_slice(&(geometry.mask_scalars_v1()? as u16).to_be_bytes());
    wire.extend_from_slice(&((geometry.rounds_v1() * 3) as u16).to_be_bytes());
    wire.extend_from_slice(&(messages.len() as u16).to_be_bytes());
    wire.extend_from_slice(&[
        ENDPOINT_VECTOR_COMMITMENTS_V1 as u8,
        ENDPOINT_GATES_V1 as u8,
        ENDPOINT_CONSTRAINTS_V1 as u8,
        geometry.coordinate_bits as u8,
    ]);
    wire.extend_from_slice(&(endpoint_core.len() as u16).to_be_bytes());
    wire.extend_from_slice(&[RHO_CHALLENGE_ORDINAL_V1 as u8, MAX_CHALLENGE_ATTEMPTS_V1]);
    wire.extend_from_slice(
        &u32::try_from(downstream_residual.len())
            .map_err(|_| RnsNativeGlobalInverseProductErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    if wire.len() != HEADER_BYTES_V1 {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    wire.extend_from_slice(messages);
    wire.extend_from_slice(endpoint_core);
    wire.extend_from_slice(downstream_residual);
    let digest = codec_digest_v1(&wire);
    wire.extend_from_slice(&digest);
    if wire.len() != total || wire.capacity() != total {
        return Err(RnsNativeGlobalInverseProductErrorV1::ResourceExhausted);
    }
    Ok(wire)
}

struct DecoderV1<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> DecoderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take_v1(&mut self, count: usize) -> Result<&'a [u8], RnsNativeGlobalInverseProductErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RnsNativeGlobalInverseProductErrorV1::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidHeader)?;
        self.cursor = end;
        Ok(value)
    }

    fn array_v1<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], RnsNativeGlobalInverseProductErrorV1> {
        self.take_v1(N)?
            .try_into()
            .map_err(|_| RnsNativeGlobalInverseProductErrorV1::InvalidHeader)
    }

    fn u8_v1(&mut self) -> Result<u8, RnsNativeGlobalInverseProductErrorV1> {
        Ok(self.array_v1::<1>()?[0])
    }

    fn u16_v1(&mut self) -> Result<u16, RnsNativeGlobalInverseProductErrorV1> {
        Ok(u16::from_be_bytes(self.array_v1()?))
    }

    fn u32_v1(&mut self) -> Result<u32, RnsNativeGlobalInverseProductErrorV1> {
        Ok(u32::from_be_bytes(self.array_v1()?))
    }
}

#[derive(Clone, Copy)]
struct ProofViewV1<'a> {
    messages: &'a [u8],
    endpoint_core: &'a [u8],
    residual: &'a [u8],
    codec_digest: [u8; DIGEST_BYTES_V1],
    codec_offset: usize,
}

impl<'a> ProofViewV1<'a> {
    fn decode_v1(
        bytes: &'a [u8],
        geometry: KernelGeometryV1,
        cap: usize,
    ) -> Result<Self, RnsNativeGlobalInverseProductErrorV1> {
        geometry.validate_v1()?;
        if bytes.len() > cap {
            return Err(RnsNativeGlobalInverseProductErrorV1::ProofCapExceeded);
        }
        let expected_message_bytes = geometry.rounds_v1() * 3 * SCALAR_BYTES_V1;
        let expected_endpoint_bytes = endpoint_core_bytes_v1(geometry)?;
        let min = HEADER_BYTES_V1
            + expected_message_bytes
            + expected_endpoint_bytes
            + MIN_DOWNSTREAM_RESIDUAL_BYTES_V1
            + CODEC_DIGEST_BYTES_V1;
        if bytes.len() < min {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidHeader);
        }
        let mut decoder = DecoderV1::new(bytes);
        if decoder.array_v1::<4>()? != MAGIC_V1
            || decoder.u8_v1()? != VERSION_V1
            || decoder.u8_v1()? != FLAGS_V1
            || usize::from(decoder.u16_v1()?) != HEADER_BYTES_V1
            || usize::try_from(decoder.u32_v1()?)
                .map_err(|_| RnsNativeGlobalInverseProductErrorV1::ArithmeticOverflow)?
                != bytes.len()
            || usize::from(decoder.u16_v1()?) != geometry.active_planes
            || usize::from(decoder.u16_v1()?) != geometry.padded_planes
            || usize::from(decoder.u16_v1()?) != geometry.coordinates
            || usize::from(decoder.u8_v1()?) != geometry.coordinate_bits
            || usize::from(decoder.u8_v1()?) != geometry.plane_bits
            || usize::from(decoder.u8_v1()?) != geometry.rounds_v1()
            || usize::from(decoder.u8_v1()?) != SUMCHECK_DEGREE_V1
            || usize::from(decoder.u16_v1()?) != geometry.mask_scalars_v1()?
            || usize::from(decoder.u16_v1()?) != geometry.rounds_v1() * 3
            || usize::from(decoder.u16_v1()?) != expected_message_bytes
            || usize::from(decoder.u8_v1()?) != ENDPOINT_VECTOR_COMMITMENTS_V1
            || usize::from(decoder.u8_v1()?) != ENDPOINT_GATES_V1
            || usize::from(decoder.u8_v1()?) != ENDPOINT_CONSTRAINTS_V1
            || usize::from(decoder.u8_v1()?) != geometry.coordinate_bits
            || usize::from(decoder.u16_v1()?) != expected_endpoint_bytes
            || u32::from(decoder.u8_v1()?) != RHO_CHALLENGE_ORDINAL_V1
            || decoder.u8_v1()? != MAX_CHALLENGE_ATTEMPTS_V1
        {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
        }
        let residual_len = usize::try_from(decoder.u32_v1()?)
            .map_err(|_| RnsNativeGlobalInverseProductErrorV1::ArithmeticOverflow)?;
        if decoder.cursor != HEADER_BYTES_V1 || residual_len == 0 {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidHeader);
        }
        let expected = HEADER_BYTES_V1
            .checked_add(expected_message_bytes)
            .and_then(|value| value.checked_add(expected_endpoint_bytes))
            .and_then(|value| value.checked_add(residual_len))
            .and_then(|value| value.checked_add(CODEC_DIGEST_BYTES_V1))
            .ok_or(RnsNativeGlobalInverseProductErrorV1::ArithmeticOverflow)?;
        if expected != bytes.len() {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidHeader);
        }
        let messages = decoder.take_v1(expected_message_bytes)?;
        let endpoint_core = decoder.take_v1(expected_endpoint_bytes)?;
        let residual = decoder.take_v1(residual_len)?;
        let codec_offset = decoder.cursor;
        let codec_digest = decoder.array_v1()?;
        if decoder.cursor != bytes.len() || codec_digest_v1(&bytes[..codec_offset]) != codec_digest
        {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidIntegrity);
        }
        Ok(Self {
            messages,
            endpoint_core,
            residual,
            codec_digest,
            codec_offset,
        })
    }
}

fn codec_digest_v1(bytes: &[u8]) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(CODEC_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(bytes);
    hash.finalize()
}

struct VerifiedKernelV1<'a> {
    residual: &'a [u8],
    rho: Scalar,
    u_sum_commitment: Point,
    sumcheck_transcript_digest: [u8; DIGEST_BYTES_V1],
    endpoint_transcript_digest: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    binding_digest: [u8; DIGEST_BYTES_V1],
}

fn verify_kernel_for_suite_v1<'a, S>(
    geometry: KernelGeometryV1,
    context: KernelContextV1,
    predecessor_binding_digest: [u8; DIGEST_BYTES_V1],
    commitments: KernelCommitmentsV1<'_>,
    wire: &'a [u8],
    cap: usize,
) -> Result<VerifiedKernelV1<'a>, RnsNativeGlobalInverseProductErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    if predecessor_binding_digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidContext);
    }
    let u_sum_commitment = derive_active_u_sum_commitment_v1(geometry, commitments.u)?;
    let view = ProofViewV1::decode_v1(wire, geometry, cap)?;
    let (mut sumcheck_state, rho) = initial_sumcheck_transcript_v1(geometry, context, commitments)?;
    let mut claim = Scalar::zero();
    let mut challenges = Vec::with_capacity(geometry.rounds_v1());
    for round in 0..geometry.rounds_v1() {
        let offset = round * MESSAGE_SCALARS_PER_ROUND_V1 * SCALAR_BYTES_V1;
        let compressed = [
            decode_scalar_v1(&view.messages[offset..offset + SCALAR_BYTES_V1])?,
            decode_scalar_v1(
                &view.messages[offset + SCALAR_BYTES_V1..offset + 2 * SCALAR_BYTES_V1],
            )?,
            decode_scalar_v1(
                &view.messages[offset + 2 * SCALAR_BYTES_V1..offset + 3 * SCALAR_BYTES_V1],
            )?,
        ];
        let polynomial = reconstruct_cubic_v1(claim, compressed);
        if polynomial[0] + evaluate_cubic_v1(polynomial, Scalar::one()) != claim {
            return Err(RnsNativeGlobalInverseProductErrorV1::InvalidSumcheck);
        }
        let challenge = absorb_sumcheck_message_v1(&mut sumcheck_state, round, compressed)?;
        claim = evaluate_cubic_v1(polynomial, challenge);
        challenges.push(challenge);
    }
    let coordinate_point = &challenges[..geometry.coordinate_bits];
    let plane_point = &challenges[geometry.coordinate_bits..];
    let z_inverse = context
        .z
        .invert()
        .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidContext)?;
    let (a_commitment, u_commitment, _) =
        folded_endpoint_commitments_v1::<S>(geometry, commitments, z_inverse, plane_point)?;
    let rho_evaluation = rho_endpoint_evaluation_v1(geometry, rho, &challenges)?;
    let endpoint_state = endpoint_initial_state_v1(
        geometry,
        &sumcheck_state,
        [a_commitment, u_commitment, commitments.inverse_product_mask],
        claim,
    )?;
    let mut endpoint_transcript = EndpointVerifierTranscriptV1::<S>::new_v1(
        endpoint_state,
        view.endpoint_core,
        endpoint_core_bytes_v1(geometry)?,
    )?;
    build_endpoint_statement_v1::<S>(
        geometry,
        a_commitment,
        u_commitment,
        commitments.inverse_product_mask,
        coordinate_point,
        &challenges,
        rho_evaluation,
        claim,
        context.z,
    )?
    .verify(&mut endpoint_transcript)?;
    let endpoint_transcript_digest = endpoint_transcript.finish_v1()?;
    let sumcheck_transcript_digest = hash_v1(&sumcheck_state);
    let mut residual_hash = Keccak256::new();
    residual_hash.update(RESIDUAL_DOMAIN_V1);
    residual_hash.update(&[VERSION_V1]);
    residual_hash.update(&predecessor_binding_digest);
    residual_hash.update(&sumcheck_transcript_digest);
    residual_hash.update(&endpoint_transcript_digest);
    residual_hash.update(&(view.residual.len() as u32).to_be_bytes());
    residual_hash.update(view.residual);
    let residual_digest = residual_hash.finalize();
    let mut binding = Keccak256::new();
    binding.update(BINDING_DOMAIN_V1);
    binding.update(&[VERSION_V1]);
    for digest in [
        manifest_digest_v1(geometry)?,
        context.pre_z_binding_digest,
        context.post_z_transcript_digest,
        predecessor_binding_digest,
        commitment_set_digest_v1(geometry, commitments)?,
        sumcheck_transcript_digest,
        endpoint_transcript_digest,
        residual_digest,
        view.codec_digest,
    ] {
        binding.update(&digest);
    }
    binding.update(&(view.codec_offset as u32).to_be_bytes());
    let binding_digest = binding.finalize();
    if [residual_digest, binding_digest].contains(&[0; DIGEST_BYTES_V1]) {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidIntegrity);
    }
    Ok(VerifiedKernelV1 {
        residual: view.residual,
        rho,
        u_sum_commitment,
        sumcheck_transcript_digest,
        endpoint_transcript_digest,
        residual_digest,
        binding_digest,
    })
}

#[allow(
    dead_code,
    reason = "the private prover is consumed by the future sole transport builder"
)]
pub(super) fn prove_rns_native_global_inverse_product_kernel_v1<P, R>(
    context: KernelContextV1,
    commitments: KernelCommitmentsV1<'_>,
    source: P,
    downstream_residual: &[u8],
    rng: &mut R,
) -> Result<Vec<u8>, RnsNativeGlobalInverseProductErrorV1>
where
    P: RnsNativeGlobalInverseProductOpeningSourceV1,
    R: ProofRandomSource,
{
    prove_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, _, _>(
        KernelGeometryV1::PRODUCTION,
        context,
        commitments,
        source,
        downstream_residual,
        rng,
    )
}

fn collect_production_commitments_v1<'source, 'proof, S>(
    previous: &super::RnsNativeGlobalLookupPostZPrerequisiteV1<'source, 'proof, S>,
) -> Result<(Vec<Point>, Vec<Point>), RnsNativeGlobalInverseProductErrorV1>
where
    S: super::ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let existing = previous.previous().previous().previous();
    let inventory = existing
        .previous()
        .previous()
        .previous()
        .previous()
        .inventory();
    let mut a = Vec::new();
    let mut u = Vec::new();
    a.try_reserve_exact(ACTIVE_PLANES_V1)
        .map_err(|_| RnsNativeGlobalInverseProductErrorV1::ResourceExhausted)?;
    u.try_reserve_exact(ACTIVE_PLANES_V1)
        .map_err(|_| RnsNativeGlobalInverseProductErrorV1::ResourceExhausted)?;

    // Exact active-plane order: D-low, S-low, Delta, small-positive,
    // small-negative, q-digit column-major, q-complement column-major.
    for group in 0..super::GROUPS_V1 {
        let radix = existing
            .existing_radix_commitments(group)
            .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidContext)?;
        a.extend_from_slice(&radix.difference_low);
    }
    for group in 0..super::GROUPS_V1 {
        let radix = existing
            .existing_radix_commitments(group)
            .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidContext)?;
        a.extend_from_slice(&radix.slack_low);
    }
    for group in 0..super::GROUPS_V1 {
        let subtraction = inventory
            .comparator_subtraction_commitments(group)
            .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidContext)?;
        a.extend_from_slice(&subtraction.difference_digits);
    }
    for block in 0..super::SMALL_BLOCKS_V1 {
        a.push(
            inventory
                .small_source_product_commitments(block)
                .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidContext)?
                .positive,
        );
    }
    for block in 0..super::SMALL_BLOCKS_V1 {
        a.push(
            inventory
                .small_source_product_commitments(block)
                .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidContext)?
                .negative_magnitude,
        );
    }
    for column in 0..super::Q_MASK_DIGITS_V1 {
        for owner in 0..super::Q_MASK_BLOCKS_V1 {
            a.push(
                inventory
                    .q_mask_linear_commitments(owner)
                    .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidContext)?
                    .digits[column],
            );
        }
    }
    for column in 0..super::Q_MASK_DIGITS_V1 {
        for owner in 0..super::Q_MASK_BLOCKS_V1 {
            a.push(
                inventory
                    .q_mask_linear_commitments(owner)
                    .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidContext)?
                    .complement_digits[column],
            );
        }
    }

    for ordinal in 0..super::EXISTING_INVERSE_POINTS_V1 {
        u.push(
            super::point_from_existing_inverse_v1(previous.existing_inverse_bytes(), ordinal)
                .map_err(|_| RnsNativeGlobalInverseProductErrorV1::InvalidPoint)?,
        );
    }
    for group in 0..super::GROUPS_V1 {
        for column in 0..super::LOW_DIGITS_V1 {
            u.push(
                inventory
                    .comparator_difference_inverse(group, column)
                    .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidContext)?,
            );
        }
    }
    for block in 0..super::SMALL_BLOCKS_V1 {
        u.push(
            inventory
                .small_source_lookup_inverses(block)
                .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidContext)?
                .0,
        );
    }
    for block in 0..super::SMALL_BLOCKS_V1 {
        u.push(
            inventory
                .small_source_lookup_inverses(block)
                .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidContext)?
                .1,
        );
    }
    for column in 0..super::Q_MASK_DIGITS_V1 {
        for owner in 0..super::Q_MASK_BLOCKS_V1 {
            u.push(
                inventory
                    .q_mask_lookup_inverses(owner)
                    .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidContext)?
                    .digit_inverses[column],
            );
        }
    }
    for column in 0..super::Q_MASK_DIGITS_V1 {
        for owner in 0..super::Q_MASK_BLOCKS_V1 {
            u.push(
                inventory
                    .q_mask_lookup_inverses(owner)
                    .ok_or(RnsNativeGlobalInverseProductErrorV1::InvalidContext)?
                    .complement_inverses[column],
            );
        }
    }
    if a.len() != ACTIVE_PLANES_V1
        || u.len() != ACTIVE_PLANES_V1
        || a.capacity() != ACTIVE_PLANES_V1
        || u.capacity() != ACTIVE_PLANES_V1
    {
        return Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry);
    }
    Ok((a, u))
}

/// Move-only evidence that every authenticated active inverse plane passed the
/// compact randomized product relation.  It deliberately carries no lookup,
/// cross-field, readiness, release, or authorization capability.
#[allow(
    missing_copy_implementations,
    reason = "the post-z commitment owner and downstream residual advance exactly once"
)]
pub(in super::super) struct RnsNativeGlobalInverseProductPrerequisiteV1<
    'source,
    'proof,
    S: super::ZkAmsMkheRnsNativeSourceSnapshotV1,
> {
    previous: super::RnsNativeGlobalLookupPostZPrerequisiteV1<'source, 'proof, S>,
    residual: &'proof [u8],
    u_sum_commitment: Point,
    rho_challenge_digest: [u8; DIGEST_BYTES_V1],
    sumcheck_transcript_digest: [u8; DIGEST_BYTES_V1],
    endpoint_transcript_digest: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    binding_digest: [u8; DIGEST_BYTES_V1],
}

#[allow(
    dead_code,
    reason = "the private inverse-product prerequisite awaits the global lookup consumer"
)]
impl<'source, 'proof, S: super::ZkAmsMkheRnsNativeSourceSnapshotV1>
    RnsNativeGlobalInverseProductPrerequisiteV1<'source, 'proof, S>
{
    pub(in super::super) const fn previous(
        &self,
    ) -> &super::RnsNativeGlobalLookupPostZPrerequisiteV1<'source, 'proof, S> {
        &self.previous
    }

    pub(in super::super) const fn residual(&self) -> &'proof [u8] {
        self.residual
    }

    pub(in super::super) const fn u_sum_commitment(&self) -> Point {
        self.u_sum_commitment
    }

    pub(in super::super) const fn rho_challenge_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.rho_challenge_digest
    }

    pub(in super::super) const fn sumcheck_transcript_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.sumcheck_transcript_digest
    }

    pub(in super::super) const fn endpoint_transcript_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.endpoint_transcript_digest
    }

    pub(in super::super) const fn residual_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.residual_digest
    }

    pub(in super::super) const fn binding_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.binding_digest
    }
}

/// Consume the sole post-`z` commitment owner and verify the compact inverse
/// product proof in its residual.
#[allow(
    dead_code,
    reason = "the private inverse-product verifier awaits declaration in the sole-z parent"
)]
pub(in super::super) fn verify_rns_native_global_inverse_product_v1<'source, 'proof, S>(
    previous: super::RnsNativeGlobalLookupPostZPrerequisiteV1<'source, 'proof, S>,
) -> Result<
    RnsNativeGlobalInverseProductPrerequisiteV1<'source, 'proof, S>,
    RnsNativeGlobalInverseProductErrorV1,
>
where
    S: super::ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let (a, u) = collect_production_commitments_v1(&previous)?;
    let context = KernelContextV1 {
        pre_z_binding_digest: previous.pre_z_binding_digest(),
        post_z_transcript_digest: previous.post_z_transcript_digest(),
        z: previous.z_challenge(),
    };
    let verified = verify_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1>(
        KernelGeometryV1::PRODUCTION,
        context,
        previous.binding_digest(),
        KernelCommitmentsV1 {
            a: &a,
            u: &u,
            inverse_product_mask: previous.inverse_product_mask(),
        },
        previous.residual(),
        PARENT_RESIDUAL_CAP_BYTES_V1,
    )?;
    Ok(RnsNativeGlobalInverseProductPrerequisiteV1 {
        residual: verified.residual,
        u_sum_commitment: verified.u_sum_commitment,
        rho_challenge_digest: rho_challenge_digest_v1(verified.rho),
        sumcheck_transcript_digest: verified.sumcheck_transcript_digest,
        endpoint_transcript_digest: verified.endpoint_transcript_digest,
        residual_digest: verified.residual_digest,
        binding_digest: verified.binding_digest,
        previous,
    })
}

#[path = "rns_native_global_membership_direct.rs"]
pub(super) mod rns_native_global_membership_direct;
pub(in super::super) use rns_native_global_membership_direct::RNS_NATIVE_GLOBAL_MEMBERSHIP_RESIDUAL_MAX_BYTES_V1;

#[cfg(test)]
#[path = "rns_native_global_inverse_product_sumcheck_tests.rs"]
mod tests;
