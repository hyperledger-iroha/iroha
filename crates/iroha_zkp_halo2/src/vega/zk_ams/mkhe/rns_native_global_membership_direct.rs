//! Direct log-derivative membership child for the compact native-RNS lookup.
//!
//! Its parent is the compact inverse-product module, whose verified token
//! establishes every active
//! `(z - A[p, v]) * U[p, v] = 1` relation.  This child proves the remaining two
//! field equalities with one 32,768-wide generalized Bulletproof:
//!
//! ```text
//! sum_v U_sum[v] = sum_y M[y] / (z - y)
//! sum_y M[y]     = 530,972,672,
//! ```
//!
//! where `U_sum[v] = sum_{p=0}^{32_407} U[p, v]`.  `U_sum` has 16,384 physical
//! coordinates and is interpreted under the 32,768-coordinate proof basis by
//! appending 16,384 literal zero coordinates.  The compact inverse proof's 360
//! virtual planes are deliberately absent: their `U = z^-1` values exist only
//! to pad that proof's plane-domain sum-check and are not lookup occurrences.
//!
//! Both input commitments are verifier-derived or already authenticated.  The
//! first is the group sum of exactly the 32,408 active inverse commitments; the
//! second is the unique pre-`z` multiplicity commitment.  Neither point is
//! serialized again in this frame.  The source boundary exposes their openings
//! exactly once: an already-aggregated 16,384-scalar `U_sum` opening and one
//! 32,768-scalar multiplicity opening.  All caller-owned destinations are
//! zeroizing before either fallible source call.
//!
//! The membership Fiat-Shamir seed binds the pre-`z` binding, post-`z`
//! transcript, and the compact inverse proof's rho, sum-check, and endpoint
//! transcript digests.  Those values are independent of this child because the
//! inverse core excludes its downstream residual.  Conversely, the seed must
//! not contain the post-`z` binding, inverse residual/binding/codec, or any
//! membership residual/binding/codec: each hashes bytes containing this core
//! and would create a transcript cycle.  The inverse binding is admitted only
//! after this core verifies, when the successor residual and binding digests
//! are computed.
//!
//! Soundness is conditioned on binding `A` and `M` before the parent samples
//! `z`, and on acceptance of the compact inverse proof.  Write `P_A` for the
//! degree-`N` occurrence polynomial and `P_T` for the degree-32,768 table
//! polynomial.  Clearing the two log derivatives gives
//!
//! ```text
//! H(X) = P_A'(X) P_T(X)
//!      - P_A(X) sum_y M[y] P_T(X)/(X-y).
//! ```
//!
//! The checked total `sum M=N` cancels the leading term, so a false relation
//! has `deg H <= N+32,768-2 = 531,005,438`.  If `H` is identically zero, every
//! actual pole lies in the table and its residue equals `M[y]`.  Actual pole
//! residues are integers in `1..=N`; the manifest pins `N=530,972,672<pT`, so
//! none vanishes by reduction.  Conditioned on an ideal uniform `z` outside
//! the table, the bad-root term is at most `531,005,438/(pT-32,768)`.  The full
//! claim additionally includes the parent's wide-reduction/bounded-rejection,
//! compact-inverse, generalized-Bulletproof, binding, and Keccak-ROM terms;
//! exhausting any 128-attempt challenge loop rejects.

use core::{fmt, marker::PhantomData};

use crate::{
    generalized_bulletproof::{
        ArithmeticCircuitStatement, ArithmeticCircuitWitness, GeneralizedBulletproofErrorV1,
        LinComb, ProofRandomSource, ProofScalar, ProofSuite, ProverTranscript, Variable,
        VectorCommitmentOpening, VerifierTranscript,
    },
    vega::{
        VEGA_T256_SCALAR_MODULUS_BE_V1, VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
        bulletproof_t256::{ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1, ZkAmsT256BulletproofSuiteV1},
        sponge::Keccak256,
    },
};

const VERSION_V1: u8 = 1;
const FLAGS_V1: u8 = 0;
const MAGIC_V1: [u8; 4] = *b"ZGMD";
const DIGEST_BYTES_V1: usize = 32;
const POINT_BYTES_V1: usize = 33;
const SCALAR_BYTES_V1: usize = 32;
const HEADER_BYTES_V1: usize = 40;
const ACTIVE_PLANES_V1: usize = 32_408;
const U_COORDINATES_V1: usize = 1 << 14;
const TABLE_VALUES_V1: usize = 1 << 15;
const LOG_N_V1: usize = 15;
const ACTIVE_LOOKUP_VALUES_V1: u64 = 530_972_672;
const RETIRED_38_LIMB_ACTIVE_LOOKUP_VALUES_V1: u64 = 520_486_912;
const VECTOR_COMMITMENTS_V1: usize = 2;
const ACTIVE_MULTIPLICATION_GATES_V1: usize = 0;
const CONSTRAINTS_V1: usize = 2;
const NI_V1: usize = 2 + 2 * (VECTOR_COMMITMENTS_V1 / 2);
const L_POLYNOMIALS_V1: usize = NI_V1 + 2;
const T_POLYNOMIALS_V1: usize = 2 * L_POLYNOMIALS_V1 - 1;
const FIXED_POINTS_V1: usize = 3 + T_POLYNOMIALS_V1 - 1;
const IPA_POINTS_V1: usize = 2 * LOG_N_V1;
const PROOF_POINTS_V1: usize = FIXED_POINTS_V1 + IPA_POINTS_V1;
const PROOF_SCALARS_V1: usize = 5;
const CORE_BYTES_V1: usize = PROOF_POINTS_V1 * POINT_BYTES_V1 + PROOF_SCALARS_V1 * SCALAR_BYTES_V1;
const CODEC_DIGEST_BYTES_V1: usize = DIGEST_BYTES_V1;
const MIN_DOWNSTREAM_RESIDUAL_BYTES_V1: usize = 1;
const OWNED_WIRE_BYTES_V1: usize = HEADER_BYTES_V1 + CORE_BYTES_V1 + CODEC_DIGEST_BYTES_V1;
const PARENT_RESIDUAL_CAP_BYTES_V1: usize =
    super::RNS_NATIVE_GLOBAL_INVERSE_PRODUCT_RESIDUAL_MAX_BYTES_V1;
pub(in super::super::super) const RNS_NATIVE_GLOBAL_MEMBERSHIP_RESIDUAL_MAX_BYTES_V1: usize =
    PARENT_RESIDUAL_CAP_BYTES_V1 - OWNED_WIRE_BYTES_V1;
const MIN_WIRE_BYTES_V1: usize = OWNED_WIRE_BYTES_V1 + MIN_DOWNSTREAM_RESIDUAL_BYTES_V1;
const MAX_CHALLENGE_ATTEMPTS_V1: u8 = 128;
const GBP_CHALLENGES_V1: usize = 4 + LOG_N_V1;

const MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-membership-direct.manifest";
const COMMITMENT_SET_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-membership-direct.commitments";
const TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-membership-direct.transcript";
const CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-membership-direct.challenge";
const CODEC_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-global-membership-direct.codec";
const RESIDUAL_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-membership-direct.residual";
const BINDING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-global-membership-direct.binding";
const GEOMETRY_LANGUAGE_V1: &[u8] = b"active-U-planes=32408;U-coordinates=16384;U-sum=coordinatewise-sum-of-exact-active-plane-order;GBP-n=32768;U-sum-high-half=literal-zero;M-commitment=canonical-T256-G[0..32768)-plus-H-mask;M-index=y0..y14-little-endian;exclude-all-360-inverse-sumcheck-virtual-planes";
const RELATION_LANGUAGE_V1: &[u8] = b"Q_z[y]=(z-y)^-1;z-notin-0..32767;Q_z-batch-inverts-all-32768-checked-nonzero-denominators-with-one-field-inversion;CG-only-constraint0=sum-v-U_sum[v]-sum-y-Q_z[y]*M[y]=0;CG-only-constraint1=sum-y-M[y]-530972672=0;zero-active-multiplication-gates;two-vector-commitments;no-scalar-commitments";
const SOURCE_LANGUAGE_V1: &[u8] = b"move-only-source;inverse-core-handoff-owns-zeroizing-U-sum-values-and-mask;take-already-aggregated-U-sum-opening-exactly-once;take-M-opening-exactly-once;caller-zeroizing-destinations-exist-before-first-fallible-call;membership-token-is-move-only-and-owns-inverse-predecessor;no-per-plane-mask-replay;no-530972672-cell-materialization";
const TRANSCRIPT_LANGUAGE_V1: &[u8] = b"challenge-seed-binds-manifest,pre-z-binding,post-z-transcript,inverse-rho-digest,inverse-sumcheck-transcript-digest,inverse-endpoint-transcript-digest,z,derived-U-sum-commitment,and-pre-z-M-commitment;exclude-post-z-binding,inverse-residual,inverse-binding,inverse-codec,membership-residual,membership-binding,and-membership-codec-from-all-challenges;admit-predecessor-inverse-binding-only-after-core-verification";
const RETIRED_LANGUAGE_V1: &[u8] = b"retired-38-limb-Q-mask-blocks=1520;retired-q-digit-plus-complement-planes=12160;retired-active-planes=31768;retired-total=520486912-is-invalid;current-40-limb-Q-mask-blocks=1600;current-q-digit-plus-complement-planes=12800;current-active-planes=32408;current-total=530972672;difference=640*16384=10485760;retired-total-is-stale-geometry-not-a-semantic-subset";
const SOUNDNESS_LANGUAGE_V1: &[u8] = b"assumptions=A-and-M-commitments-fixed-before-parent-z;SHAKE256-RFC9380-derived-T256-G/H-multigenerator-discrete-relation-and-basis-independence;generalized-BP-knowledge-soundness-in-the-Keccak-ROM;accepted-compact-inverse-fixes-U[p,v]=(z-A[p,v])^-1;table-embeddings-0..32767-are-distinct;N=530972672<pT;actual-pole-residues-are-in-1..530972672-and-therefore-nonzero-mod-pT;H(X)=P_A'(X)P_T(X)-P_A(X)*sum_y(M[y]*P_T(X)/(X-y));sum-M=N-cancels-leading-term;invalid-membership-implies-H-nonzero-and-degree<=530972672+32768-2=531005438;H-identically-zero-implies-no-outside-table-pole-and-M-residues-equal-the-actual-multiplicities;parent-z-is-first-of-at-most-128-wide-reduced-draws-outside-table;ideal-parent-z-conditioned-outside-table-error<=531005438/(pT-32768);challenge-exhaustion-fails-closed;union-parent-z-wide-reduction-and-bounded-rejection,compact-inverse,and-generalized-BP-errors;19-GBP-accepted-nonzero-wide-reduction-challenges-each-bounded-to-128-attempts;standard-Keccak-ROM-query-loss";

const DIRECT_MEMBERSHIP_RELATION_VERIFIED_V1: bool = true;
const MULTIPLICITY_NONNEGATIVE_RANGE_VERIFIED_V1: bool = false;
const CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1: bool = false;
const RELEASE_READY_V1: bool = false;

const _: () = {
    assert!(ACTIVE_PLANES_V1 * U_COORDINATES_V1 == ACTIVE_LOOKUP_VALUES_V1 as usize);
    assert!(ACTIVE_LOOKUP_VALUES_V1 - RETIRED_38_LIMB_ACTIVE_LOOKUP_VALUES_V1 == 10_485_760);
    assert!(10_485_760 == 640 * U_COORDINATES_V1);
    assert!(NI_V1 == 4);
    assert!(L_POLYNOMIALS_V1 == 6);
    assert!(T_POLYNOMIALS_V1 == 11);
    assert!(FIXED_POINTS_V1 == 13);
    assert!(IPA_POINTS_V1 == 30);
    assert!(PROOF_POINTS_V1 == 43);
    assert!(CORE_BYTES_V1 == 1_579);
    assert!(OWNED_WIRE_BYTES_V1 == 1_651);
    assert!(MIN_WIRE_BYTES_V1 == 1_652);
    assert!(PARENT_RESIDUAL_CAP_BYTES_V1 == 110_115);
    assert!(RNS_NATIVE_GLOBAL_MEMBERSHIP_RESIDUAL_MAX_BYTES_V1 == 108_464);
    assert!(GBP_CHALLENGES_V1 == 19);
    assert!(DIRECT_MEMBERSHIP_RELATION_VERIFIED_V1);
    assert!(!MULTIPLICITY_NONNEGATIVE_RANGE_VERIFIED_V1);
    assert!(!CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1);
    assert!(!RELEASE_READY_V1);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super::super) enum RnsNativeGlobalMembershipDirectErrorV1 {
    ProofCapExceeded,
    InvalidHeader,
    InvalidGeometry,
    RetiredGeometry,
    InvalidContext,
    InvalidPoint,
    InvalidScalar,
    InvalidCore,
    InvalidIntegrity,
    ChallengeExhausted,
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "the one-shot production opening source remains deliberately uninhabited"
        )
    )]
    SourceUnavailable,
    ArithmeticOverflow,
    ResourceExhausted,
}

impl fmt::Display for RnsNativeGlobalMembershipDirectErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeGlobalMembershipDirectErrorV1 {}

impl From<GeneralizedBulletproofErrorV1> for RnsNativeGlobalMembershipDirectErrorV1 {
    fn from(error: GeneralizedBulletproofErrorV1) -> Self {
        match error {
            GeneralizedBulletproofErrorV1::PointEncoding => Self::InvalidPoint,
            GeneralizedBulletproofErrorV1::ScalarEncoding => Self::InvalidScalar,
            GeneralizedBulletproofErrorV1::TranscriptChallengeExhausted => Self::ChallengeExhausted,
            GeneralizedBulletproofErrorV1::ResourceOverflow => Self::ResourceExhausted,
            _ => Self::InvalidCore,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MembershipGeometryV1 {
    active_planes: usize,
    u_coordinates: usize,
    table_values: usize,
}

impl MembershipGeometryV1 {
    const PRODUCTION: Self = Self {
        active_planes: ACTIVE_PLANES_V1,
        u_coordinates: U_COORDINATES_V1,
        table_values: TABLE_VALUES_V1,
    };

    fn validate_v1(self) -> Result<(), RnsNativeGlobalMembershipDirectErrorV1> {
        if self.active_planes == 0
            || self.active_planes > usize::from(u16::MAX)
            || self.u_coordinates == 0
            || !self.u_coordinates.is_power_of_two()
            || self.u_coordinates > self.table_values
            || self.u_coordinates > usize::from(u16::MAX)
            || self.table_values == 0
            || !self.table_values.is_power_of_two()
            || self.table_values > usize::from(u16::MAX)
        {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidGeometry);
        }
        let total = self.active_lookup_values_v1()?;
        if !u64_is_strictly_below_scalar_modulus_v1(total)
            || !u64_is_strictly_below_scalar_modulus_v1(self.table_values as u64)
        {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidGeometry);
        }
        if total == RETIRED_38_LIMB_ACTIVE_LOOKUP_VALUES_V1 {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::RetiredGeometry);
        }
        if self == Self::PRODUCTION && total != ACTIVE_LOOKUP_VALUES_V1 {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidGeometry);
        }
        Ok(())
    }

    fn active_lookup_values_v1(self) -> Result<u64, RnsNativeGlobalMembershipDirectErrorV1> {
        u64::try_from(self.active_planes)
            .ok()
            .and_then(|planes| {
                u64::try_from(self.u_coordinates)
                    .ok()
                    .and_then(|coordinates| planes.checked_mul(coordinates))
            })
            .ok_or(RnsNativeGlobalMembershipDirectErrorV1::ArithmeticOverflow)
    }

    fn log_n_v1(self) -> Result<usize, RnsNativeGlobalMembershipDirectErrorV1> {
        self.validate_v1()?;
        usize::try_from(self.table_values.trailing_zeros())
            .map_err(|_| RnsNativeGlobalMembershipDirectErrorV1::ArithmeticOverflow)
    }

    fn challenge_count_v1(self) -> Result<usize, RnsNativeGlobalMembershipDirectErrorV1> {
        self.log_n_v1()?
            .checked_add(4)
            .ok_or(RnsNativeGlobalMembershipDirectErrorV1::ArithmeticOverflow)
    }
}

fn u64_is_strictly_below_scalar_modulus_v1(value: u64) -> bool {
    let mut encoded = [0_u8; SCALAR_BYTES_V1];
    encoded[SCALAR_BYTES_V1 - core::mem::size_of::<u64>()..].copy_from_slice(&value.to_be_bytes());
    encoded.as_slice() < VEGA_T256_SCALAR_MODULUS_BE_V1.as_slice()
}

#[derive(Clone, Copy)]
pub(super) struct MembershipContextV1 {
    pub(super) pre_z_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) post_z_transcript_digest: [u8; DIGEST_BYTES_V1],
    pub(super) inverse_rho_challenge_digest: [u8; DIGEST_BYTES_V1],
    pub(super) inverse_sumcheck_transcript_digest: [u8; DIGEST_BYTES_V1],
    pub(super) inverse_endpoint_transcript_digest: [u8; DIGEST_BYTES_V1],
    pub(super) z: Scalar,
}

impl MembershipContextV1 {
    fn validate_v1(
        self,
        geometry: MembershipGeometryV1,
    ) -> Result<(), RnsNativeGlobalMembershipDirectErrorV1> {
        geometry.validate_v1()?;
        if [
            self.pre_z_binding_digest,
            self.post_z_transcript_digest,
            self.inverse_rho_challenge_digest,
            self.inverse_sumcheck_transcript_digest,
            self.inverse_endpoint_transcript_digest,
        ]
        .contains(&[0; DIGEST_BYTES_V1])
            || !challenge_outside_table_v1(self.z, geometry.table_values)?
        {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidContext);
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(super) struct MembershipCommitmentsV1<'a> {
    /// Exact active inverse commitments in the compact inverse proof's order.
    pub(super) active_u: &'a [Point],
    /// Unique pre-z multiplicity commitment under `G[0..32768), H`.
    pub(super) multiplicity: Point,
}

/// Move-only openings accepted by the direct membership prover.
///
/// `take_u_sum_opening_v1` receives exactly 16,384 destinations in production;
/// the backend appends the high-half zeros.  `take_multiplicity_opening_v1`
/// receives exactly 32,768 destinations.  Implementations must overwrite the
/// complete destination and move the Pedersen mask into the zero-initialized
/// caller slot.  Implementations must also clear every retained secret copy in
/// `Drop`.  The kernel consumes and drops the source immediately after calling
/// each method once.
pub(super) trait RnsNativeGlobalMembershipOpeningSourceV1 {
    fn take_u_sum_opening_v1(
        &mut self,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
    ) -> Result<(), RnsNativeGlobalMembershipDirectErrorV1>;

    fn take_multiplicity_opening_v1(
        &mut self,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
    ) -> Result<(), RnsNativeGlobalMembershipDirectErrorV1>;
}

/// Separate one-shot source for the pre-z multiplicity opening.  Combined
/// proving receives the active-U sum only from the inverse pending owner and
/// receives M only through this independently move-only boundary.
pub(super) trait RnsNativeGlobalMultiplicityOpeningSourceV1 {
    fn take_multiplicity_opening_v1(
        &mut self,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
    ) -> Result<(), RnsNativeGlobalMembershipDirectErrorV1>;
}

struct PendingMembershipOpeningSourceV1<M> {
    u_sum: Option<super::ActiveUSumOpeningV1>,
    multiplicity: M,
}

impl<M> RnsNativeGlobalMembershipOpeningSourceV1 for PendingMembershipOpeningSourceV1<M>
where
    M: RnsNativeGlobalMultiplicityOpeningSourceV1,
{
    fn take_u_sum_opening_v1(
        &mut self,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
    ) -> Result<(), RnsNativeGlobalMembershipDirectErrorV1> {
        self.u_sum
            .take()
            .ok_or(RnsNativeGlobalMembershipDirectErrorV1::SourceUnavailable)?
            .take_into_v1(values, commitment_mask)
            .map_err(|_| RnsNativeGlobalMembershipDirectErrorV1::SourceUnavailable)
    }

    fn take_multiplicity_opening_v1(
        &mut self,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
    ) -> Result<(), RnsNativeGlobalMembershipDirectErrorV1> {
        self.multiplicity
            .take_multiplicity_opening_v1(values, commitment_mask)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeGlobalCombinedProverErrorV1 {
    Inverse(super::RnsNativeGlobalInverseProductErrorV1),
    Membership(RnsNativeGlobalMembershipDirectErrorV1),
}

impl From<super::RnsNativeGlobalInverseProductErrorV1> for RnsNativeGlobalCombinedProverErrorV1 {
    fn from(error: super::RnsNativeGlobalInverseProductErrorV1) -> Self {
        Self::Inverse(error)
    }
}

impl From<RnsNativeGlobalMembershipDirectErrorV1> for RnsNativeGlobalCombinedProverErrorV1 {
    fn from(error: RnsNativeGlobalMembershipDirectErrorV1) -> Self {
        Self::Membership(error)
    }
}

struct SecretScalarsV1(Vec<Scalar>);

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SecretCleanupAuditV1 {
    vector_drops: usize,
    vector_nonzero_before_clear: usize,
    vector_zero_after_clear: usize,
    scalar_drops: usize,
    scalar_nonzero_before_clear: usize,
    scalar_zero_after_clear: usize,
}

#[cfg(test)]
std::thread_local! {
    static SECRET_CLEANUP_AUDIT_V1: core::cell::Cell<SecretCleanupAuditV1> =
        const { core::cell::Cell::new(SecretCleanupAuditV1 {
            vector_drops: 0,
            vector_nonzero_before_clear: 0,
            vector_zero_after_clear: 0,
            scalar_drops: 0,
            scalar_nonzero_before_clear: 0,
            scalar_zero_after_clear: 0,
        }) };
}

#[cfg(test)]
fn reset_secret_cleanup_audit_v1() {
    SECRET_CLEANUP_AUDIT_V1.with(|audit| audit.set(SecretCleanupAuditV1::default()));
}

#[cfg(test)]
fn secret_cleanup_audit_v1() -> SecretCleanupAuditV1 {
    SECRET_CLEANUP_AUDIT_V1.with(core::cell::Cell::get)
}

impl SecretScalarsV1 {
    fn try_zeroed_v1(len: usize) -> Result<Self, RnsNativeGlobalMembershipDirectErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(len)
            .map_err(|_| RnsNativeGlobalMembershipDirectErrorV1::ResourceExhausted)?;
        values.resize(len, Scalar::zero());
        Ok(Self(values))
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
        #[cfg(test)]
        let had_nonzero = self.0.iter().any(|value| !value.is_zero());
        let values = core::hint::black_box(&mut self.0);
        for value in values.iter_mut() {
            value.clear_secret();
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        #[cfg(test)]
        SECRET_CLEANUP_AUDIT_V1.with(|audit| {
            let mut value = audit.get();
            value.vector_drops += 1;
            value.vector_nonzero_before_clear += usize::from(u8::from(had_nonzero));
            value.vector_zero_after_clear +=
                usize::from(u8::from(values.iter().all(|scalar| scalar.is_zero())));
            audit.set(value);
        });
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
}

impl Drop for SecretScalarV1 {
    fn drop(&mut self) {
        #[cfg(test)]
        let had_nonzero = !self.0.is_zero();
        self.0.clear_secret();
        #[cfg(test)]
        SECRET_CLEANUP_AUDIT_V1.with(|audit| {
            let mut value = audit.get();
            value.scalar_drops += 1;
            value.scalar_nonzero_before_clear += usize::from(u8::from(had_nonzero));
            value.scalar_zero_after_clear += usize::from(u8::from(self.0.is_zero()));
            audit.set(value);
        });
    }
}

fn encode_point_v1(
    point: Point,
) -> Result<[u8; POINT_BYTES_V1], RnsNativeGlobalMembershipDirectErrorV1> {
    let mut encoded = [0_u8; POINT_BYTES_V1];
    point
        .write_non_identity_wire_bytes_ref(&mut encoded)
        .map_err(|_| RnsNativeGlobalMembershipDirectErrorV1::InvalidPoint)?;
    Ok(encoded)
}

fn hash_v1(bytes: &[u8]) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(bytes);
    hash.finalize()
}

fn append_frame_v1(
    state: &mut Vec<u8>,
    label: &[u8],
    value: &[u8],
) -> Result<(), RnsNativeGlobalMembershipDirectErrorV1> {
    state.extend_from_slice(
        &u16::try_from(label.len())
            .map_err(|_| RnsNativeGlobalMembershipDirectErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    state.extend_from_slice(label);
    state.extend_from_slice(
        &u32::try_from(value.len())
            .map_err(|_| RnsNativeGlobalMembershipDirectErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    state.extend_from_slice(value);
    Ok(())
}

fn challenge_outside_table_v1(
    challenge: Scalar,
    table_values: usize,
) -> Result<bool, RnsNativeGlobalMembershipDirectErrorV1> {
    if table_values == 0 || table_values > usize::from(u16::MAX) {
        return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidGeometry);
    }
    let bytes = challenge.to_le_bytes();
    Ok(bytes[2..].iter().any(|byte| *byte != 0)
        || usize::from(u16::from_le_bytes([bytes[0], bytes[1]])) >= table_values)
}

fn derive_nonzero_challenge_v1(
    state: &mut Vec<u8>,
    ordinal: u32,
) -> Result<Scalar, RnsNativeGlobalMembershipDirectErrorV1> {
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
    Err(RnsNativeGlobalMembershipDirectErrorV1::ChallengeExhausted)
}

fn manifest_digest_v1(
    geometry: MembershipGeometryV1,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeGlobalMembershipDirectErrorV1> {
    geometry.validate_v1()?;
    let mut hash = Keccak256::new();
    hash.update(MANIFEST_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    for value in [
        geometry.active_planes,
        geometry.u_coordinates,
        geometry.table_values,
        geometry.log_n_v1()?,
        VECTOR_COMMITMENTS_V1,
        ACTIVE_MULTIPLICATION_GATES_V1,
        CONSTRAINTS_V1,
        geometry.challenge_count_v1()?,
    ] {
        hash.update(&(value as u32).to_be_bytes());
    }
    hash.update(&geometry.active_lookup_values_v1()?.to_be_bytes());
    hash.update(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    hash.update(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);
    for language in [
        GEOMETRY_LANGUAGE_V1,
        RELATION_LANGUAGE_V1,
        SOURCE_LANGUAGE_V1,
        TRANSCRIPT_LANGUAGE_V1,
        RETIRED_LANGUAGE_V1,
        SOUNDNESS_LANGUAGE_V1,
    ] {
        hash.update(&(language.len() as u16).to_be_bytes());
        hash.update(language);
    }
    let digest = hash.finalize();
    (digest != [0; DIGEST_BYTES_V1])
        .then_some(digest)
        .ok_or(RnsNativeGlobalMembershipDirectErrorV1::InvalidIntegrity)
}

/// Sum exactly the authenticated active U commitments.  No virtual-plane or
/// coordinate-padding point is added: both paddings are literal zero here.
fn derive_u_sum_commitment_v1(
    geometry: MembershipGeometryV1,
    active_u: &[Point],
) -> Result<Point, RnsNativeGlobalMembershipDirectErrorV1> {
    geometry.validate_v1()?;
    if active_u.len() != geometry.active_planes
        || active_u.iter().copied().any(|point| point.is_identity())
    {
        return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidGeometry);
    }
    let mut sum = Point::identity();
    for point in active_u.iter().copied() {
        sum += point;
    }
    if sum.is_identity() {
        return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidPoint);
    }
    Ok(sum)
}

fn commitment_set_digest_v1(
    geometry: MembershipGeometryV1,
    u_sum: Point,
    multiplicity: Point,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeGlobalMembershipDirectErrorV1> {
    geometry.validate_v1()?;
    if u_sum.is_identity() || multiplicity.is_identity() {
        return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidPoint);
    }
    let mut hash = Keccak256::new();
    hash.update(COMMITMENT_SET_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&manifest_digest_v1(geometry)?);
    hash.update(&(geometry.active_planes as u32).to_be_bytes());
    hash.update(&(geometry.u_coordinates as u32).to_be_bytes());
    hash.update(&(geometry.table_values as u32).to_be_bytes());
    hash.update(&[1]);
    hash.update(&encode_point_v1(u_sum)?);
    hash.update(&[2]);
    hash.update(&encode_point_v1(multiplicity)?);
    let digest = hash.finalize();
    (digest != [0; DIGEST_BYTES_V1])
        .then_some(digest)
        .ok_or(RnsNativeGlobalMembershipDirectErrorV1::InvalidIntegrity)
}

fn initial_transcript_state_v1(
    geometry: MembershipGeometryV1,
    context: MembershipContextV1,
    u_sum: Point,
    multiplicity: Point,
) -> Result<Vec<u8>, RnsNativeGlobalMembershipDirectErrorV1> {
    context.validate_v1(geometry)?;
    let mut state = Vec::with_capacity(512);
    append_frame_v1(&mut state, b"domain", TRANSCRIPT_DOMAIN_V1)?;
    append_frame_v1(&mut state, b"manifest", &manifest_digest_v1(geometry)?)?;
    append_frame_v1(&mut state, b"pre-z-binding", &context.pre_z_binding_digest)?;
    append_frame_v1(
        &mut state,
        b"post-z-transcript",
        &context.post_z_transcript_digest,
    )?;
    append_frame_v1(
        &mut state,
        b"inverse-rho",
        &context.inverse_rho_challenge_digest,
    )?;
    append_frame_v1(
        &mut state,
        b"inverse-sumcheck",
        &context.inverse_sumcheck_transcript_digest,
    )?;
    append_frame_v1(
        &mut state,
        b"inverse-endpoint",
        &context.inverse_endpoint_transcript_digest,
    )?;
    append_frame_v1(&mut state, b"z", &context.z.to_le_bytes())?;
    append_frame_v1(
        &mut state,
        b"commitment-set",
        &commitment_set_digest_v1(geometry, u_sum, multiplicity)?,
    )?;
    Ok(state)
}

fn table_inverse_weights_v1(
    geometry: MembershipGeometryV1,
    z: Scalar,
) -> Result<Vec<Scalar>, RnsNativeGlobalMembershipDirectErrorV1> {
    geometry.validate_v1()?;
    if !challenge_outside_table_v1(z, geometry.table_values)? {
        return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidContext);
    }
    let mut denominators = Vec::new();
    denominators
        .try_reserve_exact(geometry.table_values)
        .map_err(|_| RnsNativeGlobalMembershipDirectErrorV1::ResourceExhausted)?;
    let mut prefixes = Vec::new();
    prefixes
        .try_reserve_exact(geometry.table_values)
        .map_err(|_| RnsNativeGlobalMembershipDirectErrorV1::ResourceExhausted)?;
    let mut product = Scalar::one();
    for y in 0..geometry.table_values {
        let denominator = z - Scalar::from_u64(y as u64);
        if denominator.is_zero() {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidContext);
        }
        prefixes.push(product);
        product *= denominator;
        denominators.push(denominator);
    }
    let mut product_inverse = product
        .invert()
        .ok_or(RnsNativeGlobalMembershipDirectErrorV1::InvalidContext)?;
    for index in (0..geometry.table_values).rev() {
        let denominator = denominators[index];
        denominators[index] = product_inverse * prefixes[index];
        product_inverse *= denominator;
    }
    if product_inverse != Scalar::one() {
        return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidContext);
    }
    Ok(denominators)
}

fn build_statement_v1<S: ProofSuite<Scalar = Scalar, Point = Point>>(
    geometry: MembershipGeometryV1,
    context: MembershipContextV1,
    u_sum: Point,
    multiplicity: Point,
) -> Result<ArithmeticCircuitStatement<'static, S>, RnsNativeGlobalMembershipDirectErrorV1> {
    context.validate_v1(geometry)?;
    let q_z = table_inverse_weights_v1(geometry, context.z)?;
    let mut log_derivative = LinComb::empty();
    for index in 0..geometry.u_coordinates {
        log_derivative = log_derivative.term(
            Scalar::one(),
            Variable::CG {
                commitment: 0,
                index,
            },
        );
    }
    for (index, weight) in q_z.into_iter().enumerate() {
        log_derivative = log_derivative.term(
            -weight,
            Variable::CG {
                commitment: 1,
                index,
            },
        );
    }
    let mut total =
        LinComb::empty().constant(-Scalar::from_u64(geometry.active_lookup_values_v1()?));
    for index in 0..geometry.table_values {
        total = total.term(
            Scalar::one(),
            Variable::CG {
                commitment: 1,
                index,
            },
        );
    }
    Ok(ArithmeticCircuitStatement::new(
        S::generators().reduce(geometry.table_values)?,
        vec![log_derivative, total],
        vec![u_sum, multiplicity],
        Vec::new(),
    )?)
}

fn core_bytes_v1(
    geometry: MembershipGeometryV1,
) -> Result<usize, RnsNativeGlobalMembershipDirectErrorV1> {
    geometry.validate_v1()?;
    let ni = 2 + 2 * (VECTOR_COMMITMENTS_V1 / 2);
    let l_polynomials = ni + 2;
    let t_polynomials = 2 * l_polynomials - 1;
    let fixed_points = 3 + t_polynomials - 1;
    let ipa_points = 2 * geometry.log_n_v1()?;
    fixed_points
        .checked_add(ipa_points)
        .and_then(|points| points.checked_mul(POINT_BYTES_V1))
        .and_then(|bytes| bytes.checked_add(PROOF_SCALARS_V1 * SCALAR_BYTES_V1))
        .ok_or(RnsNativeGlobalMembershipDirectErrorV1::ArithmeticOverflow)
}

fn wire_bytes_v1(
    geometry: MembershipGeometryV1,
    core_len: usize,
    downstream_residual_len: usize,
) -> Result<usize, RnsNativeGlobalMembershipDirectErrorV1> {
    geometry.validate_v1()?;
    if core_len != core_bytes_v1(geometry)? || downstream_residual_len == 0 {
        return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidGeometry);
    }
    let total = HEADER_BYTES_V1
        .checked_add(core_len)
        .and_then(|value| value.checked_add(downstream_residual_len))
        .and_then(|value| value.checked_add(CODEC_DIGEST_BYTES_V1))
        .ok_or(RnsNativeGlobalMembershipDirectErrorV1::ArithmeticOverflow)?;
    u32::try_from(total)
        .and_then(|_| u32::try_from(downstream_residual_len))
        .map_err(|_| RnsNativeGlobalMembershipDirectErrorV1::ArithmeticOverflow)?;
    if geometry == MembershipGeometryV1::PRODUCTION && total > PARENT_RESIDUAL_CAP_BYTES_V1 {
        return Err(RnsNativeGlobalMembershipDirectErrorV1::ProofCapExceeded);
    }
    Ok(total)
}

struct CoreProverTranscriptV1<S: ProofSuite<Scalar = Scalar, Point = Point>> {
    state: Vec<u8>,
    proof: Vec<u8>,
    challenge_ordinal: u32,
    expected_challenges: usize,
    expected_bytes: usize,
    suite: PhantomData<S>,
}

impl<S: ProofSuite<Scalar = Scalar, Point = Point>> CoreProverTranscriptV1<S> {
    fn new_v1(state: Vec<u8>, expected_challenges: usize, expected_bytes: usize) -> Self {
        Self {
            state,
            proof: Vec::with_capacity(expected_bytes),
            challenge_ordinal: 0,
            expected_challenges,
            expected_bytes,
            suite: PhantomData,
        }
    }

    fn finish_v1(
        self,
    ) -> Result<(Vec<u8>, [u8; DIGEST_BYTES_V1]), RnsNativeGlobalMembershipDirectErrorV1> {
        if self.proof.len() != self.expected_bytes
            || self.proof.capacity() != self.expected_bytes
            || usize::try_from(self.challenge_ordinal)
                .map_err(|_| RnsNativeGlobalMembershipDirectErrorV1::ArithmeticOverflow)?
                != self.expected_challenges
        {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidCore);
        }
        Ok((self.proof, hash_v1(&self.state)))
    }
}

impl<S: ProofSuite<Scalar = Scalar, Point = Point>> ProverTranscript<S>
    for CoreProverTranscriptV1<S>
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

struct CoreVerifierTranscriptV1<'a, S: ProofSuite<Scalar = Scalar, Point = Point>> {
    state: Vec<u8>,
    proof: &'a [u8],
    cursor: usize,
    challenge_ordinal: u32,
    expected_challenges: usize,
    suite: PhantomData<S>,
}

impl<'a, S: ProofSuite<Scalar = Scalar, Point = Point>> CoreVerifierTranscriptV1<'a, S> {
    fn new_v1(
        state: Vec<u8>,
        proof: &'a [u8],
        expected_challenges: usize,
        expected_bytes: usize,
    ) -> Result<Self, RnsNativeGlobalMembershipDirectErrorV1> {
        if proof.len() != expected_bytes {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidCore);
        }
        Ok(Self {
            state,
            proof,
            cursor: 0,
            challenge_ordinal: 0,
            expected_challenges,
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

    fn finish_v1(self) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeGlobalMembershipDirectErrorV1> {
        if self.cursor != self.proof.len()
            || usize::try_from(self.challenge_ordinal)
                .map_err(|_| RnsNativeGlobalMembershipDirectErrorV1::ArithmeticOverflow)?
                != self.expected_challenges
        {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidCore);
        }
        Ok(hash_v1(&self.state))
    }
}

impl<S: ProofSuite<Scalar = Scalar, Point = Point>> VerifierTranscript<S>
    for CoreVerifierTranscriptV1<'_, S>
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

#[allow(clippy::too_many_arguments)]
fn prove_kernel_for_suite_v1<S, P, R>(
    geometry: MembershipGeometryV1,
    context: MembershipContextV1,
    commitments: MembershipCommitmentsV1<'_>,
    mut source: P,
    downstream_residual: &[u8],
    rng: &mut R,
) -> Result<Vec<u8>, RnsNativeGlobalMembershipDirectErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
    P: RnsNativeGlobalMembershipOpeningSourceV1,
    R: ProofRandomSource,
{
    context.validate_v1(geometry)?;
    if downstream_residual.is_empty() {
        return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidGeometry);
    }
    let expected_core = core_bytes_v1(geometry)?;
    wire_bytes_v1(geometry, expected_core, downstream_residual.len())?;
    let u_sum_commitment = derive_u_sum_commitment_v1(geometry, commitments.active_u)?;
    if commitments.multiplicity.is_identity() {
        return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidPoint);
    }

    // Establish every retained zeroizing destination before the first source
    // call.  A partial write, later source error, or unwind therefore clears all
    // accepted secret material.
    let mut u_sum_values = SecretScalarsV1::try_zeroed_v1(geometry.u_coordinates)?;
    let mut multiplicity_values = SecretScalarsV1::try_zeroed_v1(geometry.table_values)?;
    let mut u_sum_mask = SecretScalarV1::zero_v1();
    let mut multiplicity_mask = SecretScalarV1::zero_v1();
    source.take_u_sum_opening_v1(u_sum_values.as_mut_slice_v1(), u_sum_mask.as_mut_v1())?;
    source.take_multiplicity_opening_v1(
        multiplicity_values.as_mut_slice_v1(),
        multiplicity_mask.as_mut_v1(),
    )?;
    drop(source);

    let state = initial_transcript_state_v1(
        geometry,
        context,
        u_sum_commitment,
        commitments.multiplicity,
    )?;
    let mut transcript =
        CoreProverTranscriptV1::<S>::new_v1(state, geometry.challenge_count_v1()?, expected_core);
    let witness = ArithmeticCircuitWitness::<S>::new(
        Vec::new(),
        Vec::new(),
        vec![
            VectorCommitmentOpening::take_mask_from_slot(
                u_sum_values.into_vec_v1(),
                u_sum_mask.as_mut_v1(),
            ),
            VectorCommitmentOpening::take_mask_from_slot(
                multiplicity_values.into_vec_v1(),
                multiplicity_mask.as_mut_v1(),
            ),
        ],
    )?;
    build_statement_v1::<S>(
        geometry,
        context,
        u_sum_commitment,
        commitments.multiplicity,
    )?
    .prove(rng, &mut transcript, witness)?;
    let (core, _) = transcript.finish_v1()?;
    encode_wire_v1(geometry, &core, downstream_residual)
}

fn encode_wire_v1(
    geometry: MembershipGeometryV1,
    core: &[u8],
    downstream_residual: &[u8],
) -> Result<Vec<u8>, RnsNativeGlobalMembershipDirectErrorV1> {
    let total = wire_bytes_v1(geometry, core.len(), downstream_residual.len())?;
    let mut wire = Vec::with_capacity(total);
    wire.extend_from_slice(&MAGIC_V1);
    wire.extend_from_slice(&[VERSION_V1, FLAGS_V1]);
    wire.extend_from_slice(&(HEADER_BYTES_V1 as u16).to_be_bytes());
    wire.extend_from_slice(
        &u32::try_from(total)
            .map_err(|_| RnsNativeGlobalMembershipDirectErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    wire.extend_from_slice(&(geometry.active_planes as u16).to_be_bytes());
    wire.extend_from_slice(&(geometry.u_coordinates as u16).to_be_bytes());
    wire.extend_from_slice(&(geometry.table_values as u16).to_be_bytes());
    wire.extend_from_slice(&[
        geometry.log_n_v1()? as u8,
        VECTOR_COMMITMENTS_V1 as u8,
        ACTIVE_MULTIPLICATION_GATES_V1 as u8,
        CONSTRAINTS_V1 as u8,
    ]);
    wire.extend_from_slice(
        &u16::try_from(core.len())
            .map_err(|_| RnsNativeGlobalMembershipDirectErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    wire.extend_from_slice(&[
        MAX_CHALLENGE_ATTEMPTS_V1,
        geometry.challenge_count_v1()? as u8,
        POINT_BYTES_V1 as u8,
        SCALAR_BYTES_V1 as u8,
    ]);
    wire.extend_from_slice(&geometry.active_lookup_values_v1()?.to_be_bytes());
    wire.extend_from_slice(
        &u32::try_from(downstream_residual.len())
            .map_err(|_| RnsNativeGlobalMembershipDirectErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    if wire.len() != HEADER_BYTES_V1 {
        return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidGeometry);
    }
    wire.extend_from_slice(core);
    wire.extend_from_slice(downstream_residual);
    let digest = codec_digest_v1(&wire);
    wire.extend_from_slice(&digest);
    if wire.len() != total || wire.capacity() != total {
        return Err(RnsNativeGlobalMembershipDirectErrorV1::ResourceExhausted);
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

    fn take_v1(
        &mut self,
        count: usize,
    ) -> Result<&'a [u8], RnsNativeGlobalMembershipDirectErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RnsNativeGlobalMembershipDirectErrorV1::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeGlobalMembershipDirectErrorV1::InvalidHeader)?;
        self.cursor = end;
        Ok(value)
    }

    fn array_v1<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], RnsNativeGlobalMembershipDirectErrorV1> {
        self.take_v1(N)?
            .try_into()
            .map_err(|_| RnsNativeGlobalMembershipDirectErrorV1::InvalidHeader)
    }

    fn u8_v1(&mut self) -> Result<u8, RnsNativeGlobalMembershipDirectErrorV1> {
        Ok(self.array_v1::<1>()?[0])
    }

    fn u16_v1(&mut self) -> Result<u16, RnsNativeGlobalMembershipDirectErrorV1> {
        Ok(u16::from_be_bytes(self.array_v1()?))
    }

    fn u32_v1(&mut self) -> Result<u32, RnsNativeGlobalMembershipDirectErrorV1> {
        Ok(u32::from_be_bytes(self.array_v1()?))
    }

    fn u64_v1(&mut self) -> Result<u64, RnsNativeGlobalMembershipDirectErrorV1> {
        Ok(u64::from_be_bytes(self.array_v1()?))
    }
}

#[derive(Clone, Copy)]
struct ProofViewV1<'a> {
    core: &'a [u8],
    residual: &'a [u8],
    codec_digest: [u8; DIGEST_BYTES_V1],
    codec_offset: usize,
}

impl<'a> ProofViewV1<'a> {
    fn decode_v1(
        bytes: &'a [u8],
        geometry: MembershipGeometryV1,
        cap: usize,
    ) -> Result<Self, RnsNativeGlobalMembershipDirectErrorV1> {
        geometry.validate_v1()?;
        let expected_core = core_bytes_v1(geometry)?;
        let min = HEADER_BYTES_V1
            .checked_add(expected_core)
            .and_then(|value| value.checked_add(MIN_DOWNSTREAM_RESIDUAL_BYTES_V1))
            .and_then(|value| value.checked_add(CODEC_DIGEST_BYTES_V1))
            .ok_or(RnsNativeGlobalMembershipDirectErrorV1::ArithmeticOverflow)?;
        if bytes.len() > cap {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::ProofCapExceeded);
        }
        if bytes.len() < min {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidHeader);
        }
        let mut decoder = DecoderV1::new(bytes);
        if decoder.array_v1::<4>()? != MAGIC_V1
            || decoder.u8_v1()? != VERSION_V1
            || decoder.u8_v1()? != FLAGS_V1
            || usize::from(decoder.u16_v1()?) != HEADER_BYTES_V1
            || usize::try_from(decoder.u32_v1()?)
                .map_err(|_| RnsNativeGlobalMembershipDirectErrorV1::ArithmeticOverflow)?
                != bytes.len()
            || usize::from(decoder.u16_v1()?) != geometry.active_planes
            || usize::from(decoder.u16_v1()?) != geometry.u_coordinates
            || usize::from(decoder.u16_v1()?) != geometry.table_values
            || usize::from(decoder.u8_v1()?) != geometry.log_n_v1()?
            || usize::from(decoder.u8_v1()?) != VECTOR_COMMITMENTS_V1
            || usize::from(decoder.u8_v1()?) != ACTIVE_MULTIPLICATION_GATES_V1
            || usize::from(decoder.u8_v1()?) != CONSTRAINTS_V1
            || usize::from(decoder.u16_v1()?) != expected_core
            || decoder.u8_v1()? != MAX_CHALLENGE_ATTEMPTS_V1
            || usize::from(decoder.u8_v1()?) != geometry.challenge_count_v1()?
            || usize::from(decoder.u8_v1()?) != POINT_BYTES_V1
            || usize::from(decoder.u8_v1()?) != SCALAR_BYTES_V1
        {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidGeometry);
        }
        let declared_total = decoder.u64_v1()?;
        if declared_total == RETIRED_38_LIMB_ACTIVE_LOOKUP_VALUES_V1 {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::RetiredGeometry);
        }
        if declared_total != geometry.active_lookup_values_v1()? {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidGeometry);
        }
        let residual_len = usize::try_from(decoder.u32_v1()?)
            .map_err(|_| RnsNativeGlobalMembershipDirectErrorV1::ArithmeticOverflow)?;
        if decoder.cursor != HEADER_BYTES_V1 || residual_len == 0 {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidHeader);
        }
        let expected = HEADER_BYTES_V1
            .checked_add(expected_core)
            .and_then(|value| value.checked_add(residual_len))
            .and_then(|value| value.checked_add(CODEC_DIGEST_BYTES_V1))
            .ok_or(RnsNativeGlobalMembershipDirectErrorV1::ArithmeticOverflow)?;
        if expected != bytes.len() {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidHeader);
        }
        let core = decoder.take_v1(expected_core)?;
        let residual = decoder.take_v1(residual_len)?;
        let codec_offset = decoder.cursor;
        let codec_digest = decoder.array_v1()?;
        if decoder.cursor != bytes.len() || codec_digest != codec_digest_v1(&bytes[..codec_offset])
        {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidIntegrity);
        }
        Ok(Self {
            core,
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

pub(super) struct VerifiedKernelV1<'a> {
    pub(super) residual: &'a [u8],
    pub(super) u_sum_commitment: Point,
    pub(super) transcript_digest: [u8; DIGEST_BYTES_V1],
    pub(super) residual_digest: [u8; DIGEST_BYTES_V1],
    pub(super) binding_digest: [u8; DIGEST_BYTES_V1],
}

/// Move-only evidence that the authenticated compact inverse predecessor also
/// passed the direct global-membership relation.
#[allow(
    missing_copy_implementations,
    dead_code,
    reason = "the inverse predecessor and downstream residual must advance exactly once"
)]
pub(in super::super::super) struct RnsNativeGlobalMembershipPrerequisiteV1<
    'source,
    'proof,
    S: super::super::ZkAmsMkheRnsNativeSourceSnapshotV1,
> {
    previous: super::RnsNativeGlobalInverseProductPrerequisiteV1<'source, 'proof, S>,
    residual: &'proof [u8],
    u_sum_commitment: Point,
    transcript_digest: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    binding_digest: [u8; DIGEST_BYTES_V1],
}

#[allow(
    dead_code,
    reason = "the private membership prerequisite awaits its cross-field consumer"
)]
impl<'source, 'proof, S: super::super::ZkAmsMkheRnsNativeSourceSnapshotV1>
    RnsNativeGlobalMembershipPrerequisiteV1<'source, 'proof, S>
{
    pub(in super::super::super) const fn previous(
        &self,
    ) -> &super::RnsNativeGlobalInverseProductPrerequisiteV1<'source, 'proof, S> {
        &self.previous
    }

    pub(in super::super::super) const fn residual(&self) -> &'proof [u8] {
        self.residual
    }

    pub(in super::super::super) const fn u_sum_commitment(&self) -> Point {
        self.u_sum_commitment
    }

    pub(in super::super::super) const fn transcript_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.transcript_digest
    }

    pub(in super::super::super) const fn residual_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.residual_digest
    }

    pub(in super::super::super) const fn binding_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.binding_digest
    }
}

#[allow(clippy::too_many_arguments)]
fn verify_kernel_with_u_sum_for_suite_v1<'a, S>(
    geometry: MembershipGeometryV1,
    context: MembershipContextV1,
    predecessor_inverse_binding_digest: [u8; DIGEST_BYTES_V1],
    u_sum_commitment: Point,
    multiplicity: Point,
    wire: &'a [u8],
    cap: usize,
) -> Result<VerifiedKernelV1<'a>, RnsNativeGlobalMembershipDirectErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    context.validate_v1(geometry)?;
    if predecessor_inverse_binding_digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidContext);
    }
    if u_sum_commitment.is_identity() || multiplicity.is_identity() {
        return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidPoint);
    }
    let view = ProofViewV1::decode_v1(wire, geometry, cap)?;
    let state = initial_transcript_state_v1(geometry, context, u_sum_commitment, multiplicity)?;
    let mut transcript = CoreVerifierTranscriptV1::<S>::new_v1(
        state,
        view.core,
        geometry.challenge_count_v1()?,
        core_bytes_v1(geometry)?,
    )?;
    build_statement_v1::<S>(geometry, context, u_sum_commitment, multiplicity)?
        .verify(&mut transcript)?;
    let transcript_digest = transcript.finish_v1()?;

    // The predecessor binding and all current-frame bytes enter only after
    // the generalized-Bulletproof transcript has been consumed.
    let mut residual_hash = Keccak256::new();
    residual_hash.update(RESIDUAL_DOMAIN_V1);
    residual_hash.update(&[VERSION_V1]);
    residual_hash.update(&predecessor_inverse_binding_digest);
    residual_hash.update(&transcript_digest);
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
        context.inverse_rho_challenge_digest,
        context.inverse_sumcheck_transcript_digest,
        context.inverse_endpoint_transcript_digest,
        predecessor_inverse_binding_digest,
        commitment_set_digest_v1(geometry, u_sum_commitment, multiplicity)?,
        transcript_digest,
        residual_digest,
        view.codec_digest,
    ] {
        binding.update(&digest);
    }
    binding.update(&(view.codec_offset as u32).to_be_bytes());
    let binding_digest = binding.finalize();
    if [residual_digest, binding_digest].contains(&[0; DIGEST_BYTES_V1]) {
        return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidIntegrity);
    }
    Ok(VerifiedKernelV1 {
        residual: view.residual,
        u_sum_commitment,
        transcript_digest,
        residual_digest,
        binding_digest,
    })
}

#[allow(clippy::too_many_arguments)]
fn verify_kernel_for_suite_v1<'a, S>(
    geometry: MembershipGeometryV1,
    context: MembershipContextV1,
    predecessor_inverse_binding_digest: [u8; DIGEST_BYTES_V1],
    commitments: MembershipCommitmentsV1<'_>,
    wire: &'a [u8],
    cap: usize,
) -> Result<VerifiedKernelV1<'a>, RnsNativeGlobalMembershipDirectErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    let u_sum_commitment = derive_u_sum_commitment_v1(geometry, commitments.active_u)?;
    verify_kernel_with_u_sum_for_suite_v1::<S>(
        geometry,
        context,
        predecessor_inverse_binding_digest,
        u_sum_commitment,
        commitments.multiplicity,
        wire,
        cap,
    )
}

#[allow(
    dead_code,
    reason = "the generic standalone kernel remains available for audited source adapters"
)]
pub(super) fn prove_rns_native_global_membership_direct_kernel_v1<P, R>(
    context: MembershipContextV1,
    commitments: MembershipCommitmentsV1<'_>,
    source: P,
    downstream_residual: &[u8],
    rng: &mut R,
) -> Result<Vec<u8>, RnsNativeGlobalMembershipDirectErrorV1>
where
    P: RnsNativeGlobalMembershipOpeningSourceV1,
    R: ProofRandomSource,
{
    prove_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, _, _>(
        MembershipGeometryV1::PRODUCTION,
        context,
        commitments,
        source,
        downstream_residual,
        rng,
    )
}

/// Build membership from the transcript-complete inverse core and its
/// zeroizing active-U sum handoff, then seal the inverse envelope around that
/// membership frame.  The child is therefore complete before any inverse
/// residual/codec/binding can depend on it.
#[allow(clippy::too_many_arguments)]
fn prove_combined_for_suites_v1<IS, MS, P, M, R>(
    inverse_geometry: super::KernelGeometryV1,
    membership_geometry: MembershipGeometryV1,
    inverse_context: super::KernelContextV1,
    inverse_commitments: super::KernelCommitmentsV1<'_>,
    inverse_source: P,
    multiplicity: Point,
    multiplicity_source: M,
    downstream_residual: &[u8],
    rng: &mut R,
) -> Result<Vec<u8>, RnsNativeGlobalCombinedProverErrorV1>
where
    IS: ProofSuite<Scalar = Scalar, Point = Point>,
    MS: ProofSuite<Scalar = Scalar, Point = Point>,
    P: super::RnsNativeGlobalInverseProductOpeningSourceV1,
    M: RnsNativeGlobalMultiplicityOpeningSourceV1,
    R: ProofRandomSource,
{
    inverse_geometry.validate_v1()?;
    membership_geometry.validate_v1()?;
    if inverse_geometry.active_planes != membership_geometry.active_planes
        || inverse_geometry.coordinates != membership_geometry.u_coordinates
        || inverse_commitments.u.len() != membership_geometry.active_planes
    {
        return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidGeometry.into());
    }
    let pending = super::prove_pending_kernel_for_suite_v1::<IS, _, _>(
        inverse_geometry,
        inverse_context,
        inverse_commitments,
        inverse_source,
        rng,
    )?;
    let (inverse_core, u_sum_opening) = pending.into_parts_v1();
    let membership_context = MembershipContextV1 {
        pre_z_binding_digest: inverse_context.pre_z_binding_digest,
        post_z_transcript_digest: inverse_context.post_z_transcript_digest,
        inverse_rho_challenge_digest: inverse_core.rho_challenge_digest_v1(),
        inverse_sumcheck_transcript_digest: inverse_core.sumcheck_transcript_digest_v1(),
        inverse_endpoint_transcript_digest: inverse_core.endpoint_transcript_digest_v1(),
        z: inverse_context.z,
    };
    let derived_u_sum = derive_u_sum_commitment_v1(membership_geometry, inverse_commitments.u)?;
    if derived_u_sum != inverse_core.u_sum_commitment_v1() {
        return Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidIntegrity.into());
    }
    let membership_wire = prove_kernel_for_suite_v1::<MS, _, _>(
        membership_geometry,
        membership_context,
        MembershipCommitmentsV1 {
            active_u: inverse_commitments.u,
            multiplicity,
        },
        PendingMembershipOpeningSourceV1 {
            u_sum: Some(u_sum_opening),
            multiplicity: multiplicity_source,
        },
        downstream_residual,
        rng,
    )?;
    Ok(inverse_core.seal_v1(&membership_wire)?)
}

#[allow(
    dead_code,
    clippy::too_many_arguments,
    reason = "the private combined kernel awaits an authenticated live opening source"
)]
pub(super) fn prove_rns_native_global_inverse_product_and_membership_kernels_v1<P, M, R>(
    inverse_context: super::KernelContextV1,
    inverse_commitments: super::KernelCommitmentsV1<'_>,
    inverse_source: P,
    multiplicity: Point,
    multiplicity_source: M,
    downstream_residual: &[u8],
    rng: &mut R,
) -> Result<Vec<u8>, RnsNativeGlobalCombinedProverErrorV1>
where
    P: super::RnsNativeGlobalInverseProductOpeningSourceV1,
    M: RnsNativeGlobalMultiplicityOpeningSourceV1,
    R: ProofRandomSource,
{
    prove_combined_for_suites_v1::<ZkAmsT256BulletproofSuiteV1, ZkAmsT256BulletproofSuiteV1, _, _, _>(
        super::KernelGeometryV1::PRODUCTION,
        MembershipGeometryV1::PRODUCTION,
        inverse_context,
        inverse_commitments,
        inverse_source,
        multiplicity,
        multiplicity_source,
        downstream_residual,
        rng,
    )
}

#[allow(
    dead_code,
    reason = "the generic kernel remains available for the future sole transport builder"
)]
pub(super) fn verify_rns_native_global_membership_direct_kernel_v1<'a>(
    context: MembershipContextV1,
    predecessor_inverse_binding_digest: [u8; DIGEST_BYTES_V1],
    commitments: MembershipCommitmentsV1<'_>,
    wire: &'a [u8],
) -> Result<VerifiedKernelV1<'a>, RnsNativeGlobalMembershipDirectErrorV1> {
    verify_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1>(
        MembershipGeometryV1::PRODUCTION,
        context,
        predecessor_inverse_binding_digest,
        commitments,
        wire,
        PARENT_RESIDUAL_CAP_BYTES_V1,
    )
}

/// Consume verified inverse-product evidence and verify its direct-membership
/// child.  Core challenges use only predecessor core digests; the predecessor
/// binding (which hashes this child frame) is admitted after core acceptance.
#[allow(
    dead_code,
    reason = "the private membership prerequisite awaits its cross-field consumer"
)]
pub(in super::super::super) fn verify_rns_native_global_membership_v1<'source, 'proof, S>(
    previous: super::RnsNativeGlobalInverseProductPrerequisiteV1<'source, 'proof, S>,
) -> Result<
    RnsNativeGlobalMembershipPrerequisiteV1<'source, 'proof, S>,
    RnsNativeGlobalMembershipDirectErrorV1,
>
where
    S: super::super::ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let (context, multiplicity) = {
        let post_z = previous.previous();
        (
            MembershipContextV1 {
                pre_z_binding_digest: post_z.pre_z_binding_digest(),
                post_z_transcript_digest: post_z.post_z_transcript_digest(),
                inverse_rho_challenge_digest: previous.rho_challenge_digest(),
                inverse_sumcheck_transcript_digest: previous.sumcheck_transcript_digest(),
                inverse_endpoint_transcript_digest: previous.endpoint_transcript_digest(),
                z: post_z.z_challenge(),
            },
            post_z.multiplicity(),
        )
    };
    let predecessor_inverse_binding_digest = previous.binding_digest();
    let u_sum_commitment = previous.u_sum_commitment();
    let wire = previous.residual();
    let verified = verify_kernel_with_u_sum_for_suite_v1::<ZkAmsT256BulletproofSuiteV1>(
        MembershipGeometryV1::PRODUCTION,
        context,
        predecessor_inverse_binding_digest,
        u_sum_commitment,
        multiplicity,
        wire,
        PARENT_RESIDUAL_CAP_BYTES_V1,
    )?;
    Ok(RnsNativeGlobalMembershipPrerequisiteV1 {
        previous,
        residual: verified.residual,
        u_sum_commitment: verified.u_sum_commitment,
        transcript_digest: verified.transcript_digest,
        residual_digest: verified.residual_digest,
        binding_digest: verified.binding_digest,
    })
}

#[cfg(test)]
#[path = "rns_native_global_membership_direct_tests.rs"]
mod tests;
