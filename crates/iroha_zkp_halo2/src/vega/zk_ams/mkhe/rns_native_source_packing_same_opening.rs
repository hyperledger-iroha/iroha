//! Private source/packing same-opening child for the RNS-native MKHE proof.
//!
//! This private module settles the smallest proof which
//! can link the authenticated source snapshot to the existing packing
//! commitments without serializing any of those commitments again.  The exact
//! owner order is 344 reconstructed canonical-`D` vectors followed by 1,032
//! signed `r/e0/e1` vectors.  Every vector has 16,384 T256 coordinates.
//!
//! For `o=0..1,376`, the verifier fixes the commitment `C_o` and authoritative
//! source vector `v_o` before deriving a nonzero `tau`, then computes
//!
//! ```text
//! Q = sum_o tau^o (C_o - <v_o, G>).
//! ```
//!
//! The 65-byte payload is one non-identity Schnorr nonce point `A` and one
//! scalar `z`, checking `z H = A + c Q`.  `Q` is deliberately allowed to be the
//! identity: honest derived masks may cancel.  This is a Fiat--Shamir proof of
//! knowledge, not the vacuous cyclic-group statement that some scalar opening
//! of `Q` exists.  Its meaning therefore depends on the pinned T256
//! multigenerator-binding/discrete-relation assumption and on every commitment
//! being fixed before `tau`.  If any vector opening is wrong, a nonzero error
//! polynomial of degree at most 1,375 survives except with probability at most
//! `1,375/(pT-1)`, about `2^-245.6`, plus the Schnorr, binding, wide-reduction,
//! and Keccak-ROM terms.
//!
//! The transcript is acyclic.  `tau` binds the manifest, authenticated source
//! context, the internally derived canonical replay schedule, actual point
//! root, and a typed successor-independent predecessor core: terminal
//! predecessor context binding, candidate pre-direct inventory context/root,
//! existing radix candidate root, and the successor-independent direct-core
//! digest.  It excludes the source statement anchor, final source aggregation
//! schedule, every current inventory or chain-envelope binding, the combined
//! outer bundle, and every current or downstream residual/codec/binding
//! digest.  `c` additionally binds `tau`, an
//! identity-aware encoding of `Q`, and `A`; it still excludes `z` and all
//! envelope values.  The statement anchor, final aggregation schedule, current
//! inventory, complete chain envelopes, and combined outer binding are
//! requested from the predecessor and admitted only after the equation
//! verifies.
//!
//! Production remains unavailable.  No live owner currently retains the 344
//! reconstructed `D` masks plus 1,032 signed masks, and the authenticated source
//! snapshot has no purpose-bound mutable replay path through the future
//! combined direct-plus-membership predecessor.  The generic traits below are
//! testable contracts, not evidence that either production owner exists.  This
//! child grants no composite, readiness, receipt, or release authority.
#![allow(
    dead_code,
    reason = "the private same-opening child remains fail-closed until authenticated replay and derived-mask owners exist"
)]

use core::{convert::Infallible, fmt};

use super::{
    rns_native_global_lookup_z_commitment_view::rns_native_global_inverse_product_sumcheck::RNS_NATIVE_GLOBAL_MEMBERSHIP_RESIDUAL_MAX_BYTES_V1,
    rns_native_profile::zk_ams_mkhe_rns_native_profile_manifest_v1,
    rns_native_source::ZK_AMS_MKHE_RNS_NATIVE_SOURCE_VERSION_V1,
};

use crate::{
    generalized_bulletproof::{
        GeneralizedBulletproofErrorV1, ProofRandomSource, ProofSuite, SecretMultiexpBuilder,
        random_scalar,
    },
    vega::{
        VEGA_T256_SCALAR_MODULUS_BE_V1, VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
        bulletproof_t256::{
            ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1, ZeroizingT256ScalarCopyV1,
            ZeroizingT256ScalarVecV1, ZkAmsT256BulletproofSuiteV1,
            with_borrowed_t256_scalar_encoding_v1,
        },
        sponge::Keccak256,
    },
};

const VERSION_V1: u8 = 1;
const FLAGS_V1: u8 = 0;
const MAGIC_V1: [u8; 4] = *b"ZSPO";
const DIGEST_BYTES_V1: usize = 32;
const POINT_BYTES_V1: usize = 33;
const IDENTITY_AWARE_POINT_BYTES_V1: usize = 1 + POINT_BYTES_V1;
const SCALAR_BYTES_V1: usize = 32;
const HEADER_BYTES_V1: usize = 28;
const SCHNORR_PAYLOAD_BYTES_V1: usize = POINT_BYTES_V1 + SCALAR_BYTES_V1;
const CODEC_DIGEST_BYTES_V1: usize = DIGEST_BYTES_V1;
const OWNED_WIRE_BYTES_V1: usize =
    HEADER_BYTES_V1 + SCHNORR_PAYLOAD_BYTES_V1 + CODEC_DIGEST_BYTES_V1;
const MIN_SUCCESSOR_BYTES_V1: usize = 1;
const MIN_WIRE_BYTES_V1: usize = OWNED_WIRE_BYTES_V1 + MIN_SUCCESSOR_BYTES_V1;
/// Exact residual ceiling of the membership stage. The direct frame has
/// already been charged before comparator and must not be subtracted again.
const FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1: usize =
    RNS_NATIVE_GLOBAL_MEMBERSHIP_RESIDUAL_MAX_BYTES_V1;
/// Maximum nonempty successor retained after the 125-byte owned child frame.
pub(super) const RNS_NATIVE_SOURCE_PACKING_SAME_OPENING_SUCCESSOR_MAX_BYTES_V1: usize =
    FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1 - OWNED_WIRE_BYTES_V1;

const RECORDS_V1: usize = 43;
const GROUPS_PER_RECORD_V1: usize = 8;
const DIFFERENCE_GROUPS_V1: usize = RECORDS_V1 * GROUPS_PER_RECORD_V1;
const SIGNED_ROLES_V1: usize = 3;
const PLANES_PER_SIGNED_ROLE_V1: usize = 8;
const SIGNED_OWNERS_V1: usize = RECORDS_V1 * SIGNED_ROLES_V1 * PLANES_PER_SIGNED_ROLE_V1;
const OWNERS_V1: usize = DIFFERENCE_GROUPS_V1 + SIGNED_OWNERS_V1;
const VECTOR_COORDINATES_V1: usize = 1 << 14;
const RADIX_LOW_DIGITS_V1: usize = 17;
const RADIX_BASE_V1: u64 = 1 << 15;
const MAIN_SOURCE_BLOCK_BYTES_V1: usize = 8_192;
const MAIN_SOURCE_BLOCKS_PER_RECORD_V1: usize = 896;
const DIFFERENCE_BLOCKS_PER_LOCAL_GROUP_V1: usize = 64;
const DIFFERENCE_SCALAR_BYTES_V1: usize = 32;
const DIFFERENCE_SCALARS_PER_BLOCK_V1: usize =
    MAIN_SOURCE_BLOCK_BYTES_V1 / DIFFERENCE_SCALAR_BYTES_V1;
const SIGNED_FIRST_BLOCK_V1: usize = 512;
const SIGNED_BLOCKS_PER_ROLE_V1: usize = 128;
const SIGNED_BLOCKS_PER_PLANE_V1: usize = 16;
const SIGNED_SCALAR_BYTES_V1: usize = 8;
const SIGNED_SCALARS_PER_BLOCK_V1: usize = MAIN_SOURCE_BLOCK_BYTES_V1 / SIGNED_SCALAR_BYTES_V1;
const MAX_CHALLENGE_ATTEMPTS_V1: u8 = 128;
const ERROR_POLYNOMIAL_DEGREE_V1: usize = OWNERS_V1 - 1;

const MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-packing-same-opening.manifest";
const SOURCE_CONTEXT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-packing-same-opening.source-context";
const CANONICAL_REPLAY_SCHEDULE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-packing-same-opening.canonical-replay-schedule";
const CANONICAL_SOURCE_RECEIPT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-source-receipt";
const POINT_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-packing-same-opening.point-root";
const PRE_CHALLENGE_BINDING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-packing-same-opening.pre-challenge";
const TAU_CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-packing-same-opening.tau";
const SCHNORR_CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-packing-same-opening.schnorr";
const REPLAY_RECEIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-packing-same-opening.replay-receipt";
const SCALAR_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-packing-same-opening.scalar";
const Q_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-source-packing-same-opening.Q";
const PROOF_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-packing-same-opening.proof";
const CODEC_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-source-packing-same-opening.codec";
const RESIDUAL_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-packing-same-opening.residual";
const BINDING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-packing-same-opening.binding";
const COMBINED_OUTER_BINDING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-packing-same-opening.combined-outer";

const GEOMETRY_LANGUAGE_V1: &[u8] = b"owners=1376;owner-order=D(g_abs-in-[0,344)),signed(signed-unit-in-[0,1032));g_abs=record*8+g_local;record-in-[0,43);g_local-in-[0,8);signed-unit=(record*3+role)*8+plane;role-order=r,e0,e1;plane-in-[0,8);coordinates-in-[0,16384);D-low-digits=17;D-top=bD;radix-base=2^15;Schnorr-payload=33-byte-A-plus-32-byte-z=65;header=28;codec-digest=32;owned-frame=125;future-direct-cascade-plus-membership-parent-cap=actual-membership-residual=108464;direct-frame-already-charged-before-comparator-chain;nonempty-successor-cap=108339";
const SOURCE_LANGUAGE_V1: &[u8] = b"move-only-authoritative-replay-source;provider-replay-schedule-is-a-claim-checked-against-the-internally-derived-canonical-safe-source-and-fixed-owner-order-digest;actual-D-point[g_abs]=sum(h=0..16,B^h*C_Dlow[g_abs,h])+B^17*C_bD[g_abs];D-coordinate-k=64*i+b;D-record=g_abs/8;D-g_local=g_abs%8;D-slot=record*896+g_local*64+b;D-byte-offset=32*i;D-value=canonical-32-byte-big-endian-T256-scalar;signed-unit=(record*3+role)*8+plane;signed-coordinate-k=1024*local_block+i;signed-slot=record*896+512+role*128+plane*16+local_block;signed-byte-offset=8*i;signed-value=two's-complement-8-byte-big-endian-i64;authenticated-source-range-validation-precedes-replay;MIN-arithmetic-embedding-is-negative-2^63;signed-i64-embeds-as-nonnegative-x-or-negative-unsigned_abs(x)-in-T256;one-16384-scalar-tau-aggregate-replay;caller-zeroizing-destination-exists-before-fallible-replay;no-plaintext-hash;source-finish-consumes-owner";
const MASK_LANGUAGE_V1: &[u8] = b"move-only-sequential-1376-mask-provider;provider-replay-schedule-is-a-claim-checked-against-the-internally-derived-canonical-digest-retained-by-the-prepared-relation;first-344-masks-are-derived-r_D[g_abs]=sum(h=0..16,B^h*r_Dlow[g_abs,h])+B^17*r_bD[g_abs];next-1032-are-signed-r/e0/e1-masks-in-record-role-plane-order;caller-zeroizing-slot-exists-before-every-fallible-take;provider-finish-consumes-owner;no-production-owner-currently-exists";
const TRANSCRIPT_LANGUAGE_V1: &[u8] = b"all-canonical-safe-source-points-and-digests-and-typed-successor-independent-safe-core-axes-fixed-before-tau;canonical-replay-schedule=H(fixed-owner-geometry,canonical-safe-source-axes,owner-order);safe-core-order=terminal-predecessor-context-binding,candidate-pre-direct-inventory-context,candidate-pre-direct-inventory-root,existing-radix-candidate-root,direct-core-safe-digest;pre-challenge=H(manifest,source-context-including-canonical-replay-schedule,actual-point-root,safe-core-in-declared-order);exclude-source-statement-anchor,source-final-aggregation-schedule,all-current-inventory-and-chain-envelope-bindings,combined-outer-binding,predecessor-successor/codec,and-current-proof/residual/codec/final-binding-from-tau;Q-computed-after-tau-from-authoritative-replay;Q-encoding-is-tag0||33-zero-bytes-for-identity-or-tag1||canonical-compressed33;c=H(pre-challenge,tau,identity-aware-Q,nonidentity-A);exclude-z/residual/codec/final-binding-from-c;request-and-admit-typed-statement-anchor,final-aggregation,current-inventory,complete-chain,and-combined-outer-bindings-only-after-equation-verifies;128-attempt-nonzero-wide-reduced-tau-and-c;Q-identity-accepted;A-identity-rejected";
const SOUNDNESS_LANGUAGE_V1: &[u8] = b"statement-is-Fiat-Shamir-Schnorr-proof-of-knowledge-of-aggregate-H-opening-not-vacuous-existence-of-a-discrete-log;assume-SHAKE256-RFC9380-derived-T256-G/H-multigenerator-discrete-relation-and-basis-independence,Schnorr-knowledge-soundness,and-Keccak-ROM;commitments-fixed-before-tau;false-same-opening-gives-nonzero-coordinate-error-polynomial-degree<=1375;ideal-uniform-nonzero-tau-cancellation<=1375/(pT-1)<2^-245.5;each-512-bit-wide-reduction-statistical-distance<pT/(4*2^512);bounded-rejection-exhaustion-fails-closed;union-binding,Schnorr,wide-reduction,and-ROM-terms";
const INTEGRATION_LANGUAGE_V1: &[u8] = b"insert-immediately-after-the-future-combined-rns-native-direct-plus-global-membership-predecessor;predecessor-must-return-typed-successor-independent-safe-core-and-separate-post-equation-statement-anchor,final-aggregation,current-inventory,complete-chain,and-canonical-combined-outer-bundle;no-source-statement-anchor,final-aggregation-schedule,current-inventory,or-chain-envelope-binding-is-pre-tau;production-requires-purpose-bound-mutable-forwarding-to-authenticated-RLWE-source-snapshot-and-a-1376-slot-derived-mask-owner-bound-to-the-actual-point-root-and-internally-derived-canonical-replay-schedule;legacy-344-source-order-Csrc-masks-are-not-D-packing-masks;child-is-declared-source-settled-and-non-authorizing;no-composite,readiness,receipt,or-release-flag-may-change";

const SAME_OPENING_KERNEL_IMPLEMENTED_V1: bool = true;
pub(super) const RNS_NATIVE_SOURCE_PACKING_SAME_OPENING_SOURCE_SETTLED_V1: bool = true;
const PRODUCTION_COMBINED_DIRECT_MEMBERSHIP_PREDECESSOR_AVAILABLE_V1: bool = false;
const PRODUCTION_AUTHENTICATED_REPLAY_OWNER_AVAILABLE_V1: bool = false;
const PRODUCTION_DERIVED_MASK_OWNER_AVAILABLE_V1: bool = false;
const GLOBAL_MEMBERSHIP_CHILD_DECLARED_V1: bool = true;
const COMPOSITE_ACCEPTANCE_AVAILABLE_V1: bool = false;
const RELEASE_READY_V1: bool = false;

const _: () = {
    assert!(DIFFERENCE_GROUPS_V1 == 344);
    assert!(SIGNED_OWNERS_V1 == 1_032);
    assert!(OWNERS_V1 == 1_376);
    assert!(VECTOR_COORDINATES_V1 == 16_384);
    assert!(GROUPS_PER_RECORD_V1 * DIFFERENCE_BLOCKS_PER_LOCAL_GROUP_V1 == SIGNED_FIRST_BLOCK_V1);
    assert!(
        SIGNED_FIRST_BLOCK_V1 + SIGNED_ROLES_V1 * SIGNED_BLOCKS_PER_ROLE_V1
            == MAIN_SOURCE_BLOCKS_PER_RECORD_V1
    );
    assert!(
        DIFFERENCE_BLOCKS_PER_LOCAL_GROUP_V1 * DIFFERENCE_SCALARS_PER_BLOCK_V1
            == VECTOR_COORDINATES_V1
    );
    assert!(SIGNED_BLOCKS_PER_PLANE_V1 * SIGNED_SCALARS_PER_BLOCK_V1 == VECTOR_COORDINATES_V1);
    assert!(ERROR_POLYNOMIAL_DEGREE_V1 == 1_375);
    assert!(SCHNORR_PAYLOAD_BYTES_V1 == 65);
    assert!(HEADER_BYTES_V1 == 28);
    assert!(OWNED_WIRE_BYTES_V1 == 125);
    assert!(MIN_WIRE_BYTES_V1 == 126);
    assert!(DIFFERENCE_SCALARS_PER_BLOCK_V1 == 256);
    assert!(SIGNED_SCALARS_PER_BLOCK_V1 == 1_024);
    assert!(FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1 == 108_464);
    assert!(
        FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1
            == RNS_NATIVE_GLOBAL_MEMBERSHIP_RESIDUAL_MAX_BYTES_V1
    );
    assert!(RNS_NATIVE_SOURCE_PACKING_SAME_OPENING_SUCCESSOR_MAX_BYTES_V1 == 108_339);
    assert!(IDENTITY_AWARE_POINT_BYTES_V1 == 34);
    assert!(SAME_OPENING_KERNEL_IMPLEMENTED_V1);
    assert!(RNS_NATIVE_SOURCE_PACKING_SAME_OPENING_SOURCE_SETTLED_V1);
    assert!(!PRODUCTION_COMBINED_DIRECT_MEMBERSHIP_PREDECESSOR_AVAILABLE_V1);
    assert!(!PRODUCTION_AUTHENTICATED_REPLAY_OWNER_AVAILABLE_V1);
    assert!(!PRODUCTION_DERIVED_MASK_OWNER_AVAILABLE_V1);
    assert!(GLOBAL_MEMBERSHIP_CHILD_DECLARED_V1);
    assert!(!COMPOSITE_ACCEPTANCE_AVAILABLE_V1);
    assert!(!RELEASE_READY_V1);
};

/// Fail-closed codec, source, ownership, transcript, and equation failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeSourcePackingSameOpeningErrorV1 {
    ProofCapExceeded,
    InvalidHeader,
    InvalidGeometry,
    InvalidContext,
    InvalidPoint,
    InvalidScalar,
    InvalidIntegrity,
    InvalidProof,
    ChallengeExhausted,
    SourceUnavailable,
    MaskUnavailable,
    RandomnessUnavailable,
    ArithmeticOverflow,
    ResourceExhausted,
}

impl fmt::Display for RnsNativeSourcePackingSameOpeningErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeSourcePackingSameOpeningErrorV1 {}

impl From<GeneralizedBulletproofErrorV1> for RnsNativeSourcePackingSameOpeningErrorV1 {
    fn from(error: GeneralizedBulletproofErrorV1) -> Self {
        match error {
            GeneralizedBulletproofErrorV1::PointEncoding
            | GeneralizedBulletproofErrorV1::PointIdentity
            | GeneralizedBulletproofErrorV1::CircuitProverCommitmentIdentity
            | GeneralizedBulletproofErrorV1::InnerProductRoundIdentity => Self::InvalidPoint,
            GeneralizedBulletproofErrorV1::ScalarEncoding => Self::InvalidScalar,
            GeneralizedBulletproofErrorV1::RandomnessUnavailable
            | GeneralizedBulletproofErrorV1::ProverRandomnessExhausted => {
                Self::RandomnessUnavailable
            }
            GeneralizedBulletproofErrorV1::TranscriptChallengeExhausted => Self::ChallengeExhausted,
            GeneralizedBulletproofErrorV1::ResourceOverflow => Self::ResourceExhausted,
            _ => Self::InvalidProof,
        }
    }
}

/// Typed predecessor axes which are safe to bind before `tau`.
///
/// Every field is derived by the future combined predecessor before it admits
/// this child.  None may hash, encode, or otherwise depend on the predecessor's
/// successor bytes.  In particular, these are candidate roots and the direct
/// safe-core identity, not current inventory or chain-envelope bindings which
/// contain the successor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativeSourcePackingSafeCoreV1 {
    /// Exact terminal-kernel context binding, excluding later terminal roots.
    pub(super) terminal_predecessor_context_binding_digest: [u8; DIGEST_BYTES_V1],
    /// Candidate inventory context projected before any direct/successor data.
    pub(super) candidate_pre_direct_inventory_context_digest: [u8; DIGEST_BYTES_V1],
    /// Candidate inventory root over the same pre-direct projection.
    pub(super) candidate_pre_direct_inventory_root: [u8; DIGEST_BYTES_V1],
    /// Existing-radix point root fixed before `z` and all envelopes.
    pub(super) existing_radix_candidate_root: [u8; DIGEST_BYTES_V1],
    /// Domain-separated direct-core digest excluding its successor and codec.
    pub(super) direct_core_safe_digest: [u8; DIGEST_BYTES_V1],
}

impl RnsNativeSourcePackingSafeCoreV1 {
    fn digests_v1(self) -> [[u8; DIGEST_BYTES_V1]; 5] {
        [
            self.terminal_predecessor_context_binding_digest,
            self.candidate_pre_direct_inventory_context_digest,
            self.candidate_pre_direct_inventory_root,
            self.existing_radix_candidate_root,
            self.direct_core_safe_digest,
        ]
    }
}

/// Typed statement, current-inventory, and complete-chain bindings admitted
/// only after the child equation verifies.
///
/// These values may bind the combined predecessor's successor and therefore
/// MUST NOT appear in the context, source-context digest, pre-challenge digest,
/// `tau`, `Q`, or Schnorr challenge paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativeSourcePackingCombinedOuterBindingsV1 {
    pub(super) source_statement_anchor_digest: [u8; DIGEST_BYTES_V1],
    pub(super) source_final_aggregation_schedule_digest: [u8; DIGEST_BYTES_V1],
    pub(super) enclosing_packing_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) inventory_prior_context_digest: [u8; DIGEST_BYTES_V1],
    pub(super) inventory_root: [u8; DIGEST_BYTES_V1],
    pub(super) inventory_continuation_digest: [u8; DIGEST_BYTES_V1],
    pub(super) inventory_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) direct_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) comparator_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) comparator_range_carry_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) small_sign_disjointness_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) q_mask_linear_relations_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) existing_radix_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) radix_complement_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) centering_subtraction_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) global_lookup_pre_z_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) global_lookup_post_z_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) global_inverse_product_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) global_membership_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) combined_outer_binding_digest: [u8; DIGEST_BYTES_V1],
}

impl RnsNativeSourcePackingCombinedOuterBindingsV1 {
    fn component_digests_v1(self) -> [[u8; DIGEST_BYTES_V1]; 19] {
        [
            self.source_statement_anchor_digest,
            self.source_final_aggregation_schedule_digest,
            self.enclosing_packing_binding_digest,
            self.inventory_prior_context_digest,
            self.inventory_root,
            self.inventory_continuation_digest,
            self.inventory_binding_digest,
            self.direct_binding_digest,
            self.comparator_binding_digest,
            self.comparator_range_carry_binding_digest,
            self.small_sign_disjointness_binding_digest,
            self.q_mask_linear_relations_binding_digest,
            self.existing_radix_binding_digest,
            self.radix_complement_binding_digest,
            self.centering_subtraction_binding_digest,
            self.global_lookup_pre_z_binding_digest,
            self.global_lookup_post_z_binding_digest,
            self.global_inverse_product_binding_digest,
            self.global_membership_binding_digest,
        ]
    }

    /// Canonical combined binding used by the future sibling predecessor
    /// adapter.  Keeping the construction here prevents an adapter from
    /// silently omitting one of the post-equation component axes.
    pub(super) fn canonical_combined_outer_binding_digest_v1(self) -> [u8; DIGEST_BYTES_V1] {
        let mut hash = Keccak256::new();
        hash.update(COMBINED_OUTER_BINDING_DOMAIN_V1);
        hash.update(&[VERSION_V1]);
        for digest in self.component_digests_v1() {
            hash.update(&digest);
        }
        hash.finalize()
    }

    fn digests_v1(self) -> [[u8; DIGEST_BYTES_V1]; 20] {
        let components = self.component_digests_v1();
        let mut digests = [[0_u8; DIGEST_BYTES_V1]; 20];
        digests[..components.len()].copy_from_slice(&components);
        digests[components.len()] = self.combined_outer_binding_digest;
        digests
    }

    fn validate_v1(self) -> Result<(), RnsNativeSourcePackingSameOpeningErrorV1> {
        let digests = self.digests_v1();
        if digests.contains(&[0; DIGEST_BYTES_V1])
            || digests
                .iter()
                .enumerate()
                .any(|(index, digest)| digests[index + 1..].contains(digest))
            || digests.contains(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1)
            || self.combined_outer_binding_digest
                != self.canonical_combined_outer_binding_digest_v1()
        {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext);
        }
        Ok(())
    }
}

/// Exact authenticated source axes and typed safe predecessor core known before
/// `tau` is sampled.  The replay schedule is derived internally; no statement
/// anchor, final source schedule, or enclosing successor-dependent binding is
/// accepted from the caller here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativeSourcePackingSameOpeningContextV1 {
    pub(super) profile_manifest_digest: [u8; DIGEST_BYTES_V1],
    pub(super) source_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) main_snapshot_digest: [u8; DIGEST_BYTES_V1],
    pub(super) nonce_snapshot_digest: [u8; DIGEST_BYTES_V1],
    pub(super) source_receipt_digest: [u8; DIGEST_BYTES_V1],
    pub(super) source_formula_digest: [u8; DIGEST_BYTES_V1],
    pub(super) source_mapping_digest: [u8; DIGEST_BYTES_V1],
    pub(super) safe_core: RnsNativeSourcePackingSafeCoreV1,
}

/// Safe source identities which a production replay owner must derive from its
/// authenticated snapshot and source-statement owner, never from caller bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativeSourcePackingAuthenticatedSourceAxesV1 {
    pub(super) profile_manifest_digest: [u8; DIGEST_BYTES_V1],
    pub(super) source_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) main_snapshot_digest: [u8; DIGEST_BYTES_V1],
    pub(super) nonce_snapshot_digest: [u8; DIGEST_BYTES_V1],
    pub(super) source_receipt_digest: [u8; DIGEST_BYTES_V1],
    pub(super) source_formula_digest: [u8; DIGEST_BYTES_V1],
    pub(super) source_mapping_digest: [u8; DIGEST_BYTES_V1],
}

impl RnsNativeSourcePackingAuthenticatedSourceAxesV1 {
    fn digests_v1(self) -> [[u8; DIGEST_BYTES_V1]; 7] {
        [
            self.profile_manifest_digest,
            self.source_binding_digest,
            self.main_snapshot_digest,
            self.nonce_snapshot_digest,
            self.source_receipt_digest,
            self.source_formula_digest,
            self.source_mapping_digest,
        ]
    }
}

impl RnsNativeSourcePackingSameOpeningContextV1 {
    const fn authenticated_source_axes_v1(self) -> RnsNativeSourcePackingAuthenticatedSourceAxesV1 {
        RnsNativeSourcePackingAuthenticatedSourceAxesV1 {
            profile_manifest_digest: self.profile_manifest_digest,
            source_binding_digest: self.source_binding_digest,
            main_snapshot_digest: self.main_snapshot_digest,
            nonce_snapshot_digest: self.nonce_snapshot_digest,
            source_receipt_digest: self.source_receipt_digest,
            source_formula_digest: self.source_formula_digest,
            source_mapping_digest: self.source_mapping_digest,
        }
    }

    fn validate_v1(self) -> Result<(), RnsNativeSourcePackingSameOpeningErrorV1> {
        let source_digests = self.authenticated_source_axes_v1().digests_v1();
        let safe_core_digests = self.safe_core.digests_v1();
        let mut digests = [[0_u8; DIGEST_BYTES_V1]; 12];
        digests[..source_digests.len()].copy_from_slice(&source_digests);
        digests[source_digests.len()..].copy_from_slice(&safe_core_digests);
        if digests.contains(&[0; DIGEST_BYTES_V1])
            || digests
                .iter()
                .enumerate()
                .any(|(index, digest)| digests[index + 1..].contains(digest))
            || digests.contains(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1)
            || self.profile_manifest_digest != canonical_profile_manifest_digest_v1()?
            || self.source_receipt_digest != canonical_source_receipt_digest_v1(self)
        {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext);
        }
        Ok(())
    }
}

/// Canonical signed-source role order used by the 1,032 trailing owners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum RnsNativeSignedSourceRoleV1 {
    R = 0,
    E0 = 1,
    E1 = 2,
}

impl RnsNativeSignedSourceRoleV1 {
    fn from_ordinal_v1(ordinal: usize) -> Option<Self> {
        match ordinal {
            0 => Some(Self::R),
            1 => Some(Self::E0),
            2 => Some(Self::E1),
            _ => None,
        }
    }
}

/// One coordinate in the exact 1,376-owner mask/replay order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeSourcePackingOwnerCoordinateV1 {
    Difference {
        group: u16,
    },
    Signed {
        record: u8,
        role: RnsNativeSignedSourceRoleV1,
        plane: u8,
    },
}

pub(super) fn owner_coordinate_v1(
    ordinal: usize,
) -> Result<RnsNativeSourcePackingOwnerCoordinateV1, RnsNativeSourcePackingSameOpeningErrorV1> {
    if ordinal < DIFFERENCE_GROUPS_V1 {
        return Ok(RnsNativeSourcePackingOwnerCoordinateV1::Difference {
            group: u16::try_from(ordinal)
                .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
        });
    }
    let signed = ordinal
        .checked_sub(DIFFERENCE_GROUPS_V1)
        .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?;
    if signed >= SIGNED_OWNERS_V1 {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
    }
    let per_record = SIGNED_ROLES_V1 * PLANES_PER_SIGNED_ROLE_V1;
    let record = signed / per_record;
    let within_record = signed % per_record;
    let role =
        RnsNativeSignedSourceRoleV1::from_ordinal_v1(within_record / PLANES_PER_SIGNED_ROLE_V1)
            .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?;
    let plane = within_record % PLANES_PER_SIGNED_ROLE_V1;
    Ok(RnsNativeSourcePackingOwnerCoordinateV1::Signed {
        record: u8::try_from(record)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
        role,
        plane: u8::try_from(plane)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
    })
}

/// Exact canonical-source location for one `D[g_abs][coordinate]` scalar.
///
/// `source_slot` is absolute in the 38,528-slot main source arena, not relative
/// to `record`.  `byte_offset` is relative to that 8,192-byte slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativeDifferenceSourceIndexV1 {
    pub(super) owner_ordinal: u16,
    pub(super) g_abs: u16,
    pub(super) record: u8,
    pub(super) g_local: u8,
    pub(super) coordinate: u16,
    pub(super) source_slot: u32,
    pub(super) byte_offset: u16,
}

/// Transpose the packed `D` axes into the authenticated canonical source.
///
/// For `g_abs = 8*record + g_local` and `coordinate = 64*i + b`, this returns
/// `source_slot = 896*record + 64*g_local + b` and
/// `byte_offset = 32*i`.
pub(super) fn difference_source_index_v1(
    g_abs: usize,
    coordinate: usize,
) -> Result<RnsNativeDifferenceSourceIndexV1, RnsNativeSourcePackingSameOpeningErrorV1> {
    if g_abs >= DIFFERENCE_GROUPS_V1 || coordinate >= VECTOR_COORDINATES_V1 {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
    }
    let record = g_abs / GROUPS_PER_RECORD_V1;
    let g_local = g_abs % GROUPS_PER_RECORD_V1;
    let scalar_in_block = coordinate / DIFFERENCE_BLOCKS_PER_LOCAL_GROUP_V1;
    let block_in_group = coordinate % DIFFERENCE_BLOCKS_PER_LOCAL_GROUP_V1;
    let source_slot = record
        .checked_mul(MAIN_SOURCE_BLOCKS_PER_RECORD_V1)
        .and_then(|base| {
            g_local
                .checked_mul(DIFFERENCE_BLOCKS_PER_LOCAL_GROUP_V1)
                .and_then(|local| base.checked_add(local))
        })
        .and_then(|base| base.checked_add(block_in_group))
        .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?;
    let byte_offset = scalar_in_block
        .checked_mul(DIFFERENCE_SCALAR_BYTES_V1)
        .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?;
    Ok(RnsNativeDifferenceSourceIndexV1 {
        owner_ordinal: u16::try_from(g_abs)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
        g_abs: u16::try_from(g_abs)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
        record: u8::try_from(record)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
        g_local: u8::try_from(g_local)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
        coordinate: u16::try_from(coordinate)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
        source_slot: u32::try_from(source_slot)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
        byte_offset: u16::try_from(byte_offset)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
    })
}

/// Decode the canonical 32-byte big-endian scalar stored at a `D` index.
pub(super) fn difference_scalar_from_be_bytes_v1(
    encoded: [u8; DIFFERENCE_SCALAR_BYTES_V1],
) -> Result<Scalar, RnsNativeSourcePackingSameOpeningErrorV1> {
    Scalar::from_be_bytes_exact(encoded)
        .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidScalar)
}

/// Exact signed-source location for one trailing owner coordinate.
///
/// `signed_unit` is local to the 1,032 signed owners; `owner_ordinal` includes
/// the 344 leading `D` owners.  `source_slot` is absolute in the main arena and
/// `byte_offset` is relative to that 8,192-byte slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativeSignedSourceIndexV1 {
    pub(super) owner_ordinal: u16,
    pub(super) signed_unit: u16,
    pub(super) record: u8,
    pub(super) role: RnsNativeSignedSourceRoleV1,
    pub(super) plane: u8,
    pub(super) coordinate: u16,
    pub(super) local_block: u8,
    pub(super) coefficient_in_block: u16,
    pub(super) source_slot: u32,
    pub(super) byte_offset: u16,
}

/// Transpose one signed owner/coordinate into the authenticated source.
///
/// For `signed_unit = (record*3 + role)*8 + plane` and
/// `coordinate = 1024*local_block + i`, this returns
/// `source_slot = 896*record + 512 + 128*role + 16*plane + local_block` and
/// `byte_offset = 8*i`.
pub(super) fn signed_source_index_v1(
    signed_unit: usize,
    coordinate: usize,
) -> Result<RnsNativeSignedSourceIndexV1, RnsNativeSourcePackingSameOpeningErrorV1> {
    if signed_unit >= SIGNED_OWNERS_V1 || coordinate >= VECTOR_COORDINATES_V1 {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
    }
    let owners_per_record = SIGNED_ROLES_V1 * PLANES_PER_SIGNED_ROLE_V1;
    let record = signed_unit / owners_per_record;
    let within_record = signed_unit % owners_per_record;
    let role_ordinal = within_record / PLANES_PER_SIGNED_ROLE_V1;
    let role = RnsNativeSignedSourceRoleV1::from_ordinal_v1(role_ordinal)
        .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?;
    let plane = within_record % PLANES_PER_SIGNED_ROLE_V1;
    let local_block = coordinate / SIGNED_SCALARS_PER_BLOCK_V1;
    let coefficient_in_block = coordinate % SIGNED_SCALARS_PER_BLOCK_V1;
    let source_slot = record
        .checked_mul(MAIN_SOURCE_BLOCKS_PER_RECORD_V1)
        .and_then(|base| base.checked_add(SIGNED_FIRST_BLOCK_V1))
        .and_then(|base| {
            role_ordinal
                .checked_mul(SIGNED_BLOCKS_PER_ROLE_V1)
                .and_then(|role_base| base.checked_add(role_base))
        })
        .and_then(|base| {
            plane
                .checked_mul(SIGNED_BLOCKS_PER_PLANE_V1)
                .and_then(|plane_base| base.checked_add(plane_base))
        })
        .and_then(|base| base.checked_add(local_block))
        .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?;
    let byte_offset = coefficient_in_block
        .checked_mul(SIGNED_SCALAR_BYTES_V1)
        .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?;
    Ok(RnsNativeSignedSourceIndexV1 {
        owner_ordinal: u16::try_from(DIFFERENCE_GROUPS_V1 + signed_unit)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
        signed_unit: u16::try_from(signed_unit)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
        record: u8::try_from(record)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
        role,
        plane: u8::try_from(plane)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
        coordinate: u16::try_from(coordinate)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
        local_block: u8::try_from(local_block)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
        coefficient_in_block: u16::try_from(coefficient_in_block)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
        source_slot: u32::try_from(source_slot)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
        byte_offset: u16::try_from(byte_offset)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
    })
}

/// Embed one exact two's-complement, big-endian `i64` source value in T256.
///
/// `unsigned_abs` is intentional: it maps `i64::MIN` to magnitude `2^63`
/// without overflow before applying the field negation.
pub(super) fn signed_scalar_from_twos_complement_be_i64_v1(
    encoded: [u8; SIGNED_SCALAR_BYTES_V1],
) -> Scalar {
    let signed = i64::from_be_bytes(encoded);
    let magnitude = Scalar::from_u64(signed.unsigned_abs());
    if signed < 0 { -magnitude } else { magnitude }
}

/// Public completion receipt for the single aggregate source replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativeSourcePackingReplayReceiptV1 {
    pub(super) source_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) canonical_replay_schedule_digest: [u8; DIGEST_BYTES_V1],
    pub(super) owner_count: u16,
    pub(super) coordinates: u16,
}

impl RnsNativeSourcePackingReplayReceiptV1 {
    fn expected_v1(
        context: RnsNativeSourcePackingSameOpeningContextV1,
    ) -> Result<Self, RnsNativeSourcePackingSameOpeningErrorV1> {
        Ok(Self {
            source_binding_digest: context.source_binding_digest,
            canonical_replay_schedule_digest: canonical_replay_schedule_digest_v1(context)?,
            owner_count: OWNERS_V1 as u16,
            coordinates: VECTOR_COORDINATES_V1 as u16,
        })
    }
}

/// One-shot authenticated point and numeric replay source.
///
/// A production implementation must own, not merely digest, the authenticated
/// source snapshot and exact packing/inventory point view.  Its schedule getter
/// is only a claim checked against the internally derived canonical schedule.
/// The aggregate replay must clear `destination`, then add `tau^o v_o` in the owner order returned by
/// [`owner_coordinate_v1`], using [`difference_source_index_v1`],
/// [`difference_scalar_from_be_bytes_v1`], [`signed_source_index_v1`], and
/// [`signed_scalar_from_twos_complement_be_i64_v1`] as the sole numeric
/// transpose.  `destination` already has exactly 16,384 zero scalars under an
/// unwind-safe owner before this fallible call begins.
pub(super) trait RnsNativeSourcePackingAggregateReplayV1: Sized {
    fn authenticated_source_axes_v1(&self) -> RnsNativeSourcePackingAuthenticatedSourceAxesV1;

    fn canonical_replay_schedule_digest_v1(&self) -> [u8; DIGEST_BYTES_V1];

    fn difference_low_commitment_v1(
        &self,
        group: usize,
        digit: usize,
    ) -> Result<Point, RnsNativeSourcePackingSameOpeningErrorV1>;

    fn difference_top_commitment_v1(
        &self,
        group: usize,
    ) -> Result<Point, RnsNativeSourcePackingSameOpeningErrorV1>;

    fn signed_commitment_v1(
        &self,
        record: usize,
        role: RnsNativeSignedSourceRoleV1,
        plane: usize,
    ) -> Result<Point, RnsNativeSourcePackingSameOpeningErrorV1>;

    fn replay_tau_aggregate_v1(
        &mut self,
        tau: Scalar,
        destination: &mut ZeroizingT256ScalarVecV1,
    ) -> Result<RnsNativeSourcePackingReplayReceiptV1, RnsNativeSourcePackingSameOpeningErrorV1>;

    fn finish_v1(
        self,
    ) -> Result<RnsNativeSourcePackingReplayReceiptV1, RnsNativeSourcePackingSameOpeningErrorV1>;
}

/// Completion receipt for the sequential derived-mask provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativeSourcePackingMaskReceiptV1 {
    pub(super) opening_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) point_root: [u8; DIGEST_BYTES_V1],
    pub(super) canonical_replay_schedule_digest: [u8; DIGEST_BYTES_V1],
    pub(super) owner_count: u16,
}

/// Move-only sequential provider for exactly 1,376 already-derived masks.
///
/// The first 344 values are reconstructed `D` masks, not the legacy
/// source-ordered `Csrc` masks.  The remaining 1,032 values are signed
/// `r/e0/e1` masks in record/role/plane order.  Implementations must erase every
/// retained secret in `Drop`; this trait is always passed by value and never
/// returned on an error.
pub(super) trait RnsNativeSourcePackingDerivedMaskSourceV1: Sized {
    fn opening_binding_digest_v1(&self) -> [u8; DIGEST_BYTES_V1];

    fn point_root_v1(&self) -> [u8; DIGEST_BYTES_V1];

    fn canonical_replay_schedule_digest_v1(&self) -> [u8; DIGEST_BYTES_V1];

    fn take_next_mask_v1(
        &mut self,
        expected: RnsNativeSourcePackingOwnerCoordinateV1,
        destination: &mut Scalar,
    ) -> Result<(), RnsNativeSourcePackingSameOpeningErrorV1>;

    fn finish_v1(
        self,
    ) -> Result<RnsNativeSourcePackingMaskReceiptV1, RnsNativeSourcePackingSameOpeningErrorV1>;
}

/// Move-only view of the future combined direct-plus-membership predecessor.
///
/// The typed safe core MUST be complete and successor-independent, so it is
/// safe to bind before `tau`.  The typed outer bundle binds the enclosing
/// source statement/final schedule, current inventory, complete chain, and
/// combined predecessor envelopes, including their successor, and is requested
/// only after this child's Schnorr equation
/// verifies.  The concrete implementation must be added at the eventual
/// integration site, where the private combined token is visible; this private
/// module intentionally cannot widen its visibility.
pub(super) trait RnsNativeSourcePackingCombinedDirectMembershipPredecessorV1<'proof>:
    Sized
{
    fn same_opening_successor_v1(&self) -> &'proof [u8];

    fn successor_independent_safe_core_v1(&self) -> RnsNativeSourcePackingSafeCoreV1;

    fn combined_outer_bindings_v1(&self) -> RnsNativeSourcePackingCombinedOuterBindingsV1;
}

/// Deliberately uninhabited production ownership seal.
///
/// Replacing any field requires a separately audited adapter and declaration
/// change.  Generic fixtures do not inhabit this type.
struct ProductionOwnersUnavailableV1 {
    combined_direct_membership_predecessor_adapter: Infallible,
    authenticated_aggregate_replay_owner: Infallible,
    derived_mask_owner: Infallible,
}

struct ZeroizingScalarSlotV1(Scalar);

impl ZeroizingScalarSlotV1 {
    const fn zero_v1() -> Self {
        Self(Scalar::zero())
    }

    fn as_ref(&self) -> &Scalar {
        &self.0
    }

    fn as_mut(&mut self) -> &mut Scalar {
        &mut self.0
    }
}

impl Drop for ZeroizingScalarSlotV1 {
    fn drop(&mut self) {
        self.0.clear_secret();
        #[cfg(test)]
        ZEROIZING_MASK_SLOT_DROPS_V1.with(|drops| drops.set(drops.get().saturating_add(1)));
    }
}

#[cfg(test)]
std::thread_local! {
    static ZEROIZING_MASK_SLOT_DROPS_V1: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

#[cfg(test)]
fn zeroizing_mask_slot_drop_count_v1() -> usize {
    ZEROIZING_MASK_SLOT_DROPS_V1.with(core::cell::Cell::get)
}

struct ZeroizingPointV1(Point);

impl ZeroizingPointV1 {
    fn take(point: &mut Point) -> Self {
        let owned = Self(*point);
        point.clear_secret();
        owned
    }

    fn as_ref(&self) -> &Point {
        &self.0
    }
}

impl Drop for ZeroizingPointV1 {
    fn drop(&mut self) {
        self.0.clear_secret();
    }
}

struct CommitmentSetV1 {
    owners: Vec<Point>,
    point_root: [u8; DIGEST_BYTES_V1],
}

struct PreparedRelationV1 {
    manifest_digest: [u8; DIGEST_BYTES_V1],
    source_context_digest: [u8; DIGEST_BYTES_V1],
    canonical_replay_schedule_digest: [u8; DIGEST_BYTES_V1],
    point_root: [u8; DIGEST_BYTES_V1],
    replay_receipt_digest: [u8; DIGEST_BYTES_V1],
    pre_challenge_binding_digest: [u8; DIGEST_BYTES_V1],
    tau: Scalar,
    tau_digest: [u8; DIGEST_BYTES_V1],
    q: ZeroizingPointV1,
    q_digest: [u8; DIGEST_BYTES_V1],
}

fn canonical_profile_manifest_digest_v1()
-> Result<[u8; DIGEST_BYTES_V1], RnsNativeSourcePackingSameOpeningErrorV1> {
    let manifest = zk_ams_mkhe_rns_native_profile_manifest_v1()
        .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext)?;
    manifest
        .validate()
        .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext)?;
    Ok(manifest.manifest_digest)
}

fn canonical_source_receipt_digest_v1(
    context: RnsNativeSourcePackingSameOpeningContextV1,
) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(CANONICAL_SOURCE_RECEIPT_DOMAIN_V1);
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_SOURCE_VERSION_V1]);
    hash.update(&context.source_binding_digest);
    hash.update(&context.main_snapshot_digest);
    hash.update(&context.nonce_snapshot_digest);
    hash.finalize()
}

fn manifest_digest_v1() -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(MANIFEST_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&(DIFFERENCE_GROUPS_V1 as u16).to_be_bytes());
    hash.update(&(SIGNED_OWNERS_V1 as u16).to_be_bytes());
    hash.update(&(OWNERS_V1 as u16).to_be_bytes());
    hash.update(&(VECTOR_COORDINATES_V1 as u16).to_be_bytes());
    hash.update(&(RADIX_LOW_DIGITS_V1 as u8).to_be_bytes());
    hash.update(&RADIX_BASE_V1.to_be_bytes());
    hash.update(&(ERROR_POLYNOMIAL_DEGREE_V1 as u16).to_be_bytes());
    hash.update(&(HEADER_BYTES_V1 as u16).to_be_bytes());
    hash.update(&(SCHNORR_PAYLOAD_BYTES_V1 as u16).to_be_bytes());
    hash.update(&(CODEC_DIGEST_BYTES_V1 as u16).to_be_bytes());
    hash.update(&(OWNED_WIRE_BYTES_V1 as u16).to_be_bytes());
    hash.update(&(FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1 as u32).to_be_bytes());
    hash.update(
        &(RNS_NATIVE_SOURCE_PACKING_SAME_OPENING_SUCCESSOR_MAX_BYTES_V1 as u32).to_be_bytes(),
    );
    hash.update(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    hash.update(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);
    for language in [
        GEOMETRY_LANGUAGE_V1,
        SOURCE_LANGUAGE_V1,
        MASK_LANGUAGE_V1,
        TRANSCRIPT_LANGUAGE_V1,
        SOUNDNESS_LANGUAGE_V1,
        INTEGRATION_LANGUAGE_V1,
    ] {
        hash.update(&(language.len() as u32).to_be_bytes());
        hash.update(language);
    }
    hash.finalize()
}

/// Derive the schedule claim which future replay and mask owners must retain.
///
/// This is visible to sibling adapters so they never have to duplicate the
/// purpose-bound owner-order hash or substitute the unsafe final aggregation
/// schedule.
pub(super) fn canonical_replay_schedule_digest_v1(
    context: RnsNativeSourcePackingSameOpeningContextV1,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSourcePackingSameOpeningErrorV1> {
    context.validate_v1()?;
    let mut hash = Keccak256::new();
    hash.update(CANONICAL_REPLAY_SCHEDULE_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    let manifest_digest = manifest_digest_v1();
    hash.update(&manifest_digest);
    for digest in [
        context.profile_manifest_digest,
        context.source_binding_digest,
        context.main_snapshot_digest,
        context.nonce_snapshot_digest,
        context.source_receipt_digest,
        context.source_formula_digest,
        context.source_mapping_digest,
    ] {
        hash.update(&digest);
    }
    hash.update(&(DIFFERENCE_GROUPS_V1 as u16).to_be_bytes());
    hash.update(&(SIGNED_OWNERS_V1 as u16).to_be_bytes());
    hash.update(&(OWNERS_V1 as u16).to_be_bytes());
    hash.update(&(VECTOR_COORDINATES_V1 as u16).to_be_bytes());
    for ordinal in 0..OWNERS_V1 {
        hash.update(&(ordinal as u16).to_be_bytes());
        match owner_coordinate_v1(ordinal)? {
            RnsNativeSourcePackingOwnerCoordinateV1::Difference { group } => {
                hash.update(&[0]);
                hash.update(&group.to_be_bytes());
            }
            RnsNativeSourcePackingOwnerCoordinateV1::Signed {
                record,
                role,
                plane,
            } => {
                hash.update(&[1, record, role as u8, plane]);
            }
        }
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1]
        || digest == ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1
        || digest == manifest_digest
        || [
            context.profile_manifest_digest,
            context.source_binding_digest,
            context.main_snapshot_digest,
            context.nonce_snapshot_digest,
            context.source_receipt_digest,
            context.source_formula_digest,
            context.source_mapping_digest,
        ]
        .contains(&digest)
    {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn source_context_digest_v1(
    context: RnsNativeSourcePackingSameOpeningContextV1,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSourcePackingSameOpeningErrorV1> {
    context.validate_v1()?;
    let mut hash = Keccak256::new();
    hash.update(SOURCE_CONTEXT_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    // The typed predecessor safe core has its own final positions in the
    // pre-challenge binding and is not folded into the authenticated source
    // context.
    hash.update(&context.profile_manifest_digest);
    hash.update(&context.source_binding_digest);
    hash.update(&context.main_snapshot_digest);
    hash.update(&context.nonce_snapshot_digest);
    hash.update(&context.source_receipt_digest);
    hash.update(&context.source_formula_digest);
    hash.update(&context.source_mapping_digest);
    hash.update(&canonical_replay_schedule_digest_v1(context)?);
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn non_identity_point_bytes_v1(
    point: &Point,
) -> Result<[u8; POINT_BYTES_V1], RnsNativeSourcePackingSameOpeningErrorV1> {
    let mut encoded = [0_u8; POINT_BYTES_V1];
    point
        .write_non_identity_wire_bytes_ref(&mut encoded)
        .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidPoint)?;
    Ok(encoded)
}

fn identity_aware_point_bytes_v1(
    point: &Point,
) -> Result<[u8; IDENTITY_AWARE_POINT_BYTES_V1], RnsNativeSourcePackingSameOpeningErrorV1> {
    let mut encoded = [0_u8; IDENTITY_AWARE_POINT_BYTES_V1];
    if (*point).is_identity() {
        encoded[0] = 0;
        return Ok(encoded);
    }
    encoded[0] = 1;
    encoded[1..].copy_from_slice(&non_identity_point_bytes_v1(point)?);
    Ok(encoded)
}

fn collect_commitments_v1<P: RnsNativeSourcePackingAggregateReplayV1>(
    source: &P,
) -> Result<CommitmentSetV1, RnsNativeSourcePackingSameOpeningErrorV1> {
    let mut owners = Vec::new();
    owners
        .try_reserve_exact(OWNERS_V1)
        .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::ResourceExhausted)?;
    let mut hash = Keccak256::new();
    hash.update(POINT_ROOT_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&(DIFFERENCE_GROUPS_V1 as u16).to_be_bytes());
    hash.update(&(SIGNED_OWNERS_V1 as u16).to_be_bytes());
    hash.update(&(OWNERS_V1 as u16).to_be_bytes());
    hash.update(&(VECTOR_COORDINATES_V1 as u16).to_be_bytes());
    hash.update(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);

    let radix = Scalar::from_u64(RADIX_BASE_V1);
    for group in 0..DIFFERENCE_GROUPS_V1 {
        hash.update(&(group as u16).to_be_bytes());
        hash.update(&[0]);
        let mut reconstructed = Point::identity();
        let mut weight = Scalar::one();
        for digit in 0..RADIX_LOW_DIGITS_V1 {
            let point = source.difference_low_commitment_v1(group, digit)?;
            hash.update(&[1, digit as u8]);
            hash.update(&non_identity_point_bytes_v1(&point)?);
            reconstructed += point.mul_scalar(weight);
            weight *= radix;
        }
        let top = source.difference_top_commitment_v1(group)?;
        hash.update(&[2, RADIX_LOW_DIGITS_V1 as u8]);
        hash.update(&non_identity_point_bytes_v1(&top)?);
        reconstructed += top.mul_scalar(weight);
        hash.update(&[3]);
        hash.update(&identity_aware_point_bytes_v1(&reconstructed)?);
        owners.push(reconstructed);
    }

    for record in 0..RECORDS_V1 {
        for role_ordinal in 0..SIGNED_ROLES_V1 {
            let role = RnsNativeSignedSourceRoleV1::from_ordinal_v1(role_ordinal)
                .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?;
            for plane in 0..PLANES_PER_SIGNED_ROLE_V1 {
                let ordinal = DIFFERENCE_GROUPS_V1
                    + (record * SIGNED_ROLES_V1 + role_ordinal) * PLANES_PER_SIGNED_ROLE_V1
                    + plane;
                if owner_coordinate_v1(ordinal)?
                    != (RnsNativeSourcePackingOwnerCoordinateV1::Signed {
                        record: record as u8,
                        role,
                        plane: plane as u8,
                    })
                {
                    return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
                }
                let point = source.signed_commitment_v1(record, role, plane)?;
                hash.update(&(ordinal as u16).to_be_bytes());
                hash.update(&[1, record as u8, role as u8, plane as u8]);
                hash.update(&non_identity_point_bytes_v1(&point)?);
                owners.push(point);
            }
        }
    }
    if owners.len() != OWNERS_V1 {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
    }
    let point_root = hash.finalize();
    if point_root == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidIntegrity);
    }
    Ok(CommitmentSetV1 { owners, point_root })
}

fn pre_challenge_binding_digest_v1(
    manifest_digest: [u8; DIGEST_BYTES_V1],
    source_context_digest: [u8; DIGEST_BYTES_V1],
    point_root: [u8; DIGEST_BYTES_V1],
    safe_core: RnsNativeSourcePackingSafeCoreV1,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSourcePackingSameOpeningErrorV1> {
    if [manifest_digest, source_context_digest, point_root].contains(&[0; DIGEST_BYTES_V1])
        || safe_core.digests_v1().contains(&[0; DIGEST_BYTES_V1])
    {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext);
    }
    let mut hash = Keccak256::new();
    hash.update(PRE_CHALLENGE_BINDING_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&manifest_digest);
    hash.update(&source_context_digest);
    hash.update(&point_root);
    hash.update(&safe_core.terminal_predecessor_context_binding_digest);
    hash.update(&safe_core.candidate_pre_direct_inventory_context_digest);
    hash.update(&safe_core.candidate_pre_direct_inventory_root);
    hash.update(&safe_core.existing_radix_candidate_root);
    hash.update(&safe_core.direct_core_safe_digest);
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn derive_tau_v1(
    pre_challenge_binding_digest: [u8; DIGEST_BYTES_V1],
) -> Result<Scalar, RnsNativeSourcePackingSameOpeningErrorV1> {
    for attempt in 0..MAX_CHALLENGE_ATTEMPTS_V1 {
        let mut low = Keccak256::new();
        low.update(TAU_CHALLENGE_DOMAIN_V1);
        low.update(&[VERSION_V1, 0]);
        low.update(&pre_challenge_binding_digest);
        low.update(&[attempt]);
        let mut high = Keccak256::new();
        high.update(TAU_CHALLENGE_DOMAIN_V1);
        high.update(&[VERSION_V1, 1]);
        high.update(&pre_challenge_binding_digest);
        high.update(&[attempt]);
        let mut wide = [0_u8; 64];
        wide[..32].copy_from_slice(&low.finalize());
        wide[32..].copy_from_slice(&high.finalize());
        let challenge = Scalar::from_uniform_le_bytes_ref(&wide);
        wide.fill(0);
        if !challenge.is_zero() {
            return Ok(challenge);
        }
    }
    Err(RnsNativeSourcePackingSameOpeningErrorV1::ChallengeExhausted)
}

fn derive_schnorr_challenge_v1(
    pre_challenge_binding_digest: [u8; DIGEST_BYTES_V1],
    tau: Scalar,
    q: &Point,
    a: &Point,
) -> Result<Scalar, RnsNativeSourcePackingSameOpeningErrorV1> {
    let q = identity_aware_point_bytes_v1(q)?;
    let a = non_identity_point_bytes_v1(a)?;
    for attempt in 0..MAX_CHALLENGE_ATTEMPTS_V1 {
        let mut low = Keccak256::new();
        low.update(SCHNORR_CHALLENGE_DOMAIN_V1);
        low.update(&[VERSION_V1, 0]);
        low.update(&pre_challenge_binding_digest);
        low.update(&tau.to_le_bytes());
        low.update(&q);
        low.update(&a);
        low.update(&[attempt]);
        let mut high = Keccak256::new();
        high.update(SCHNORR_CHALLENGE_DOMAIN_V1);
        high.update(&[VERSION_V1, 1]);
        high.update(&pre_challenge_binding_digest);
        high.update(&tau.to_le_bytes());
        high.update(&q);
        high.update(&a);
        high.update(&[attempt]);
        let mut wide = [0_u8; 64];
        wide[..32].copy_from_slice(&low.finalize());
        wide[32..].copy_from_slice(&high.finalize());
        let challenge = Scalar::from_uniform_le_bytes_ref(&wide);
        wide.fill(0);
        if !challenge.is_zero() {
            return Ok(challenge);
        }
    }
    Err(RnsNativeSourcePackingSameOpeningErrorV1::ChallengeExhausted)
}

fn scalar_digest_v1(
    label: u8,
    scalar: Scalar,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSourcePackingSameOpeningErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(SCALAR_DIGEST_DOMAIN_V1);
    hash.update(&[VERSION_V1, label]);
    hash.update(&scalar.to_le_bytes());
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn q_digest_v1(
    q: &Point,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSourcePackingSameOpeningErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(Q_DIGEST_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&identity_aware_point_bytes_v1(q)?);
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn replay_receipt_digest_v1(
    receipt: RnsNativeSourcePackingReplayReceiptV1,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSourcePackingSameOpeningErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(REPLAY_RECEIPT_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&receipt.source_binding_digest);
    hash.update(&receipt.canonical_replay_schedule_digest);
    hash.update(&receipt.owner_count.to_be_bytes());
    hash.update(&receipt.coordinates.to_be_bytes());
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn zero_aggregate_values_v1()
-> Result<ZeroizingT256ScalarVecV1, RnsNativeSourcePackingSameOpeningErrorV1> {
    let mut values = ZeroizingT256ScalarVecV1::try_with_exact_capacity(VECTOR_COORDINATES_V1)?;
    for _ in 0..VECTOR_COORDINATES_V1 {
        values.push(Scalar::zero());
    }
    if values.len() != VECTOR_COORDINATES_V1 {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
    }
    Ok(values)
}

fn prepare_relation_v1<P: RnsNativeSourcePackingAggregateReplayV1>(
    context: RnsNativeSourcePackingSameOpeningContextV1,
    mut source: P,
) -> Result<PreparedRelationV1, RnsNativeSourcePackingSameOpeningErrorV1> {
    context.validate_v1()?;
    let canonical_replay_schedule_digest = canonical_replay_schedule_digest_v1(context)?;
    if source.authenticated_source_axes_v1() != context.authenticated_source_axes_v1()
        || source.canonical_replay_schedule_digest_v1() != canonical_replay_schedule_digest
    {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext);
    }
    let manifest_digest = manifest_digest_v1();
    let source_context_digest = source_context_digest_v1(context)?;
    let commitment_set = collect_commitments_v1(&source)?;
    let pre_challenge_binding_digest = pre_challenge_binding_digest_v1(
        manifest_digest,
        source_context_digest,
        commitment_set.point_root,
        context.safe_core,
    )?;
    let tau = derive_tau_v1(pre_challenge_binding_digest)?;

    // The zeroizing 16,384-scalar owner exists before the first fallible replay
    // call and therefore clears partial writes on error or unwind.
    let mut aggregate_values = zero_aggregate_values_v1()?;
    let expected_receipt = RnsNativeSourcePackingReplayReceiptV1::expected_v1(context)?;
    let replay_receipt = source.replay_tau_aggregate_v1(tau, &mut aggregate_values)?;
    if aggregate_values.len() != VECTOR_COORDINATES_V1 || replay_receipt != expected_receipt {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext);
    }
    let finish_receipt = source.finish_v1()?;
    if finish_receipt != replay_receipt {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext);
    }

    let generators = ZkAmsT256BulletproofSuiteV1::generators();
    if generators.g_bold.len() < VECTOR_COORDINATES_V1 || generators.h.is_identity() {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
    }
    let mut value_terms =
        SecretMultiexpBuilder::<ZkAmsT256BulletproofSuiteV1>::new(VECTOR_COORDINATES_V1)?;
    for (value, generator) in aggregate_values
        .as_slice()
        .iter()
        .zip(&generators.g_bold[..VECTOR_COORDINATES_V1])
    {
        value_terms.push(value, generator)?;
    }
    let value_commitment = value_terms.evaluate()?;

    let mut commitment_aggregate = Point::identity();
    let mut power = Scalar::one();
    for point in &commitment_set.owners {
        commitment_aggregate += point.mul_scalar(power);
        power *= tau;
    }
    let mut q_slot = commitment_aggregate - *value_commitment.expose_ref();
    let q = ZeroizingPointV1::take(&mut q_slot);
    let q_digest = q_digest_v1(q.as_ref())?;
    Ok(PreparedRelationV1 {
        manifest_digest,
        source_context_digest,
        canonical_replay_schedule_digest,
        point_root: commitment_set.point_root,
        replay_receipt_digest: replay_receipt_digest_v1(replay_receipt)?,
        pre_challenge_binding_digest,
        tau,
        tau_digest: scalar_digest_v1(0, tau)?,
        q,
        q_digest,
    })
}

fn aggregate_masks_v1<M: RnsNativeSourcePackingDerivedMaskSourceV1>(
    tau: Scalar,
    point_root: [u8; DIGEST_BYTES_V1],
    canonical_replay_schedule_digest: [u8; DIGEST_BYTES_V1],
    mut source: M,
) -> Result<ZeroizingT256ScalarCopyV1, RnsNativeSourcePackingSameOpeningErrorV1> {
    let opening_binding_digest = source.opening_binding_digest_v1();
    if opening_binding_digest == [0; DIGEST_BYTES_V1]
        || source.point_root_v1() != point_root
        || source.canonical_replay_schedule_digest_v1() != canonical_replay_schedule_digest
    {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext);
    }
    let mut aggregate = ZeroizingT256ScalarCopyV1::new(Scalar::zero());
    let mut power = Scalar::one();
    for ordinal in 0..OWNERS_V1 {
        // This owner exists before the provider is called.  A provider which
        // writes and then errors or unwinds cannot strand the written scalar.
        let mut mask = ZeroizingScalarSlotV1::zero_v1();
        source.take_next_mask_v1(owner_coordinate_v1(ordinal)?, mask.as_mut())?;
        aggregate.add_product_assign(&power, mask.as_ref());
        power *= tau;
    }
    let receipt = source.finish_v1()?;
    if receipt
        != (RnsNativeSourcePackingMaskReceiptV1 {
            opening_binding_digest,
            point_root,
            canonical_replay_schedule_digest,
            owner_count: OWNERS_V1 as u16,
        })
    {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext);
    }
    Ok(aggregate)
}

fn sample_nonzero_scalar_v1<R: ProofRandomSource>(
    rng: &mut R,
) -> Result<ZeroizingT256ScalarCopyV1, RnsNativeSourcePackingSameOpeningErrorV1> {
    for _ in 0..MAX_CHALLENGE_ATTEMPTS_V1 {
        let sampled = random_scalar::<Scalar, _>(rng)?;
        if !sampled.expose_ref().is_zero() {
            return Ok(ZeroizingT256ScalarCopyV1::new(*sampled.expose_ref()));
        }
    }
    Err(RnsNativeSourcePackingSameOpeningErrorV1::RandomnessUnavailable)
}

fn proof_digest_v1(
    a_bytes: &[u8; POINT_BYTES_V1],
    z_bytes: &[u8; SCALAR_BYTES_V1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSourcePackingSameOpeningErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(PROOF_DIGEST_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(a_bytes);
    hash.update(z_bytes);
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn codec_digest_v1(bytes: &[u8]) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(CODEC_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(bytes);
    hash.finalize()
}

fn encode_frame_v1(
    a: &Point,
    z: &Scalar,
    downstream_residual: &[u8],
) -> Result<Vec<u8>, RnsNativeSourcePackingSameOpeningErrorV1> {
    if a.is_identity()
        || downstream_residual.is_empty()
        || downstream_residual.len() > RNS_NATIVE_SOURCE_PACKING_SAME_OPENING_SUCCESSOR_MAX_BYTES_V1
    {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
    }
    let total_len = OWNED_WIRE_BYTES_V1
        .checked_add(downstream_residual.len())
        .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?;
    let total_len_u32 = u32::try_from(total_len)
        .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?;
    let residual_len_u32 = u32::try_from(downstream_residual.len())
        .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?;
    let a_bytes = non_identity_point_bytes_v1(a)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(total_len)
        .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::ResourceExhausted)?;
    bytes.extend_from_slice(&MAGIC_V1);
    bytes.extend_from_slice(&[VERSION_V1, FLAGS_V1]);
    bytes.extend_from_slice(&(HEADER_BYTES_V1 as u16).to_be_bytes());
    bytes.extend_from_slice(&total_len_u32.to_be_bytes());
    bytes.extend_from_slice(&(OWNERS_V1 as u16).to_be_bytes());
    bytes.extend_from_slice(&(DIFFERENCE_GROUPS_V1 as u16).to_be_bytes());
    bytes.extend_from_slice(&(SIGNED_OWNERS_V1 as u16).to_be_bytes());
    bytes.extend_from_slice(&(VECTOR_COORDINATES_V1 as u16).to_be_bytes());
    bytes.extend_from_slice(&[
        POINT_BYTES_V1 as u8,
        SCALAR_BYTES_V1 as u8,
        MAX_CHALLENGE_ATTEMPTS_V1,
        0,
    ]);
    bytes.extend_from_slice(&residual_len_u32.to_be_bytes());
    bytes.extend_from_slice(&a_bytes);
    with_borrowed_t256_scalar_encoding_v1(z, |z_bytes| bytes.extend_from_slice(z_bytes));
    bytes.extend_from_slice(downstream_residual);
    let codec_digest = codec_digest_v1(&bytes);
    bytes.extend_from_slice(&codec_digest);
    if bytes.len() != total_len {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
    }
    Ok(bytes)
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
    ) -> Result<&'a [u8], RnsNativeSourcePackingSameOpeningErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::InvalidHeader)?;
        self.cursor = end;
        Ok(value)
    }

    fn array_v1<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], RnsNativeSourcePackingSameOpeningErrorV1> {
        self.take_v1(N)?
            .try_into()
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidHeader)
    }

    fn u8_v1(&mut self) -> Result<u8, RnsNativeSourcePackingSameOpeningErrorV1> {
        self.take_v1(1)?
            .first()
            .copied()
            .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::InvalidHeader)
    }

    fn u16_v1(&mut self) -> Result<u16, RnsNativeSourcePackingSameOpeningErrorV1> {
        Ok(u16::from_be_bytes(self.array_v1()?))
    }

    fn u32_v1(&mut self) -> Result<u32, RnsNativeSourcePackingSameOpeningErrorV1> {
        Ok(u32::from_be_bytes(self.array_v1()?))
    }
}

struct FrameViewV1<'a> {
    a: Point,
    z: Scalar,
    a_bytes: [u8; POINT_BYTES_V1],
    z_bytes: [u8; SCALAR_BYTES_V1],
    residual: &'a [u8],
    codec_digest: [u8; DIGEST_BYTES_V1],
    codec_offset: usize,
}

impl<'a> FrameViewV1<'a> {
    fn decode_v1(
        bytes: &'a [u8],
        cap: usize,
    ) -> Result<Self, RnsNativeSourcePackingSameOpeningErrorV1> {
        if cap > FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1
            || bytes.len() > cap
            || bytes.len() > FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1
        {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::ProofCapExceeded);
        }
        if bytes.len() < MIN_WIRE_BYTES_V1 {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidHeader);
        }
        let mut decoder = DecoderV1::new(bytes);
        if decoder.array_v1::<4>()? != MAGIC_V1
            || decoder.u8_v1()? != VERSION_V1
            || decoder.u8_v1()? != FLAGS_V1
            || usize::from(decoder.u16_v1()?) != HEADER_BYTES_V1
        {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidHeader);
        }
        let total_len = usize::try_from(decoder.u32_v1()?)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidHeader)?;
        if total_len != bytes.len()
            || usize::from(decoder.u16_v1()?) != OWNERS_V1
            || usize::from(decoder.u16_v1()?) != DIFFERENCE_GROUPS_V1
            || usize::from(decoder.u16_v1()?) != SIGNED_OWNERS_V1
            || usize::from(decoder.u16_v1()?) != VECTOR_COORDINATES_V1
            || usize::from(decoder.u8_v1()?) != POINT_BYTES_V1
            || usize::from(decoder.u8_v1()?) != SCALAR_BYTES_V1
            || decoder.u8_v1()? != MAX_CHALLENGE_ATTEMPTS_V1
            || decoder.u8_v1()? != 0
        {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidHeader);
        }
        let residual_len = usize::try_from(decoder.u32_v1()?)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidHeader)?;
        if residual_len == 0
            || residual_len > RNS_NATIVE_SOURCE_PACKING_SAME_OPENING_SUCCESSOR_MAX_BYTES_V1
            || OWNED_WIRE_BYTES_V1
                .checked_add(residual_len)
                .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?
                != bytes.len()
            || decoder.cursor != HEADER_BYTES_V1
        {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidHeader);
        }
        let a_bytes = decoder.array_v1()?;
        let z_bytes = decoder.array_v1()?;
        let residual = decoder.take_v1(residual_len)?;
        let codec_offset = decoder.cursor;
        let codec_digest = decoder.array_v1()?;
        if decoder.cursor != bytes.len() || codec_digest != codec_digest_v1(&bytes[..codec_offset])
        {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidIntegrity);
        }
        let a = Point::from_non_identity_wire_bytes_exact(&a_bytes)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidPoint)?;
        if a.is_identity() {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidPoint);
        }
        let z = Scalar::from_le_bytes_exact(z_bytes)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidScalar)?;
        Ok(Self {
            a,
            z,
            a_bytes,
            z_bytes,
            residual,
            codec_digest,
            codec_offset,
        })
    }
}

/// Build the generic same-opening child around a nonempty downstream residual.
///
/// This function intentionally accepts only abstract one-shot owners.  There is
/// no production adapter or exported proof constructor.
pub(super) fn prove_rns_native_source_packing_same_opening_kernel_v1<P, M, R>(
    context: RnsNativeSourcePackingSameOpeningContextV1,
    replay_source: P,
    mask_source: M,
    downstream_residual: &[u8],
    rng: &mut R,
) -> Result<Vec<u8>, RnsNativeSourcePackingSameOpeningErrorV1>
where
    P: RnsNativeSourcePackingAggregateReplayV1,
    M: RnsNativeSourcePackingDerivedMaskSourceV1,
    R: ProofRandomSource,
{
    let prepared = prepare_relation_v1(context, replay_source)?;
    let aggregate_mask = aggregate_masks_v1(
        prepared.tau,
        prepared.point_root,
        prepared.canonical_replay_schedule_digest,
        mask_source,
    )?;
    // The source and mask capabilities have both been consumed before fresh
    // Schnorr entropy is requested.  An entropy error cannot leave a retryable
    // opening capability behind.
    let kappa = sample_nonzero_scalar_v1(rng)?;
    let hiding_generator = ZkAmsT256BulletproofSuiteV1::generators().h;
    if hiding_generator.is_identity() {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidPoint);
    }
    let mut a_slot = hiding_generator.mul_scalar(kappa.get());
    if a_slot.is_identity() {
        a_slot.clear_secret();
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidPoint);
    }
    let a = ZeroizingPointV1::take(&mut a_slot);
    let challenge = derive_schnorr_challenge_v1(
        prepared.pre_challenge_binding_digest,
        prepared.tau,
        prepared.q.as_ref(),
        a.as_ref(),
    )?;
    let mut z_slot = kappa.get() + challenge * aggregate_mask.get();
    let z = ZeroizingT256ScalarCopyV1::take(&mut z_slot);
    encode_frame_v1(a.as_ref(), z.as_ref(), downstream_residual)
}

struct VerifiedKernelV1<'a> {
    residual: &'a [u8],
    manifest_digest: [u8; DIGEST_BYTES_V1],
    source_context_digest: [u8; DIGEST_BYTES_V1],
    point_root: [u8; DIGEST_BYTES_V1],
    replay_receipt_digest: [u8; DIGEST_BYTES_V1],
    pre_challenge_binding_digest: [u8; DIGEST_BYTES_V1],
    tau_digest: [u8; DIGEST_BYTES_V1],
    q_digest: [u8; DIGEST_BYTES_V1],
    proof_digest: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    binding_digest: [u8; DIGEST_BYTES_V1],
}

/// Equation-verified state which still has no combined outer binding.
///
/// Keeping this state separate makes it impossible for final-envelope data to
/// affect `tau`, `c`, or the same-opening equation.
struct EquationVerifiedKernelV1<'a> {
    residual: &'a [u8],
    manifest_digest: [u8; DIGEST_BYTES_V1],
    source_context_digest: [u8; DIGEST_BYTES_V1],
    point_root: [u8; DIGEST_BYTES_V1],
    replay_receipt_digest: [u8; DIGEST_BYTES_V1],
    pre_challenge_binding_digest: [u8; DIGEST_BYTES_V1],
    tau_digest: [u8; DIGEST_BYTES_V1],
    q_digest: [u8; DIGEST_BYTES_V1],
    proof_digest: [u8; DIGEST_BYTES_V1],
    codec_digest: [u8; DIGEST_BYTES_V1],
    codec_offset: usize,
}

fn verify_equation_kernel_v1<'a, P: RnsNativeSourcePackingAggregateReplayV1>(
    context: RnsNativeSourcePackingSameOpeningContextV1,
    replay_source: P,
    wire: &'a [u8],
    cap: usize,
) -> Result<EquationVerifiedKernelV1<'a>, RnsNativeSourcePackingSameOpeningErrorV1> {
    // Reject malformed/cap-exceeding frames before allocating the 16,384-scalar
    // replay destination or touching the one-shot source.
    let frame = FrameViewV1::decode_v1(wire, cap)?;
    let prepared = prepare_relation_v1(context, replay_source)?;
    let challenge = derive_schnorr_challenge_v1(
        prepared.pre_challenge_binding_digest,
        prepared.tau,
        prepared.q.as_ref(),
        &frame.a,
    )?;
    let hiding_generator = ZkAmsT256BulletproofSuiteV1::generators().h;
    if hiding_generator.is_identity()
        || hiding_generator.mul_scalar(frame.z)
            != frame.a + (*prepared.q.as_ref()).mul_scalar(challenge)
    {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidProof);
    }

    // The response digest is computed only after the equation verifies, but the
    // combined outer binding is still unavailable at this stage.
    let proof_digest = proof_digest_v1(&frame.a_bytes, &frame.z_bytes)?;
    Ok(EquationVerifiedKernelV1 {
        residual: frame.residual,
        manifest_digest: prepared.manifest_digest,
        source_context_digest: prepared.source_context_digest,
        point_root: prepared.point_root,
        replay_receipt_digest: prepared.replay_receipt_digest,
        pre_challenge_binding_digest: prepared.pre_challenge_binding_digest,
        tau_digest: prepared.tau_digest,
        q_digest: prepared.q_digest,
        proof_digest,
        codec_digest: frame.codec_digest,
        codec_offset: frame.codec_offset,
    })
}

fn finalize_verified_kernel_v1<'a>(
    equation_verified: EquationVerifiedKernelV1<'a>,
    combined_outer_bindings: RnsNativeSourcePackingCombinedOuterBindingsV1,
) -> Result<VerifiedKernelV1<'a>, RnsNativeSourcePackingSameOpeningErrorV1> {
    combined_outer_bindings.validate_v1()?;
    // Only this post-equation phase admits the statement anchor, final source
    // aggregation schedule, current inventory, complete chain envelopes,
    // downstream residual, codec, and final frame offsets.
    let mut residual_hash = Keccak256::new();
    residual_hash.update(RESIDUAL_DOMAIN_V1);
    residual_hash.update(&[VERSION_V1]);
    for digest in combined_outer_bindings.digests_v1() {
        residual_hash.update(&digest);
    }
    residual_hash.update(&equation_verified.proof_digest);
    residual_hash.update(&(equation_verified.residual.len() as u32).to_be_bytes());
    residual_hash.update(equation_verified.residual);
    let residual_digest = residual_hash.finalize();
    let mut binding = Keccak256::new();
    binding.update(BINDING_DOMAIN_V1);
    binding.update(&[VERSION_V1]);
    for digest in [
        equation_verified.manifest_digest,
        equation_verified.source_context_digest,
        equation_verified.point_root,
        equation_verified.replay_receipt_digest,
        equation_verified.pre_challenge_binding_digest,
        equation_verified.tau_digest,
        equation_verified.q_digest,
        equation_verified.proof_digest,
        residual_digest,
        equation_verified.codec_digest,
    ] {
        binding.update(&digest);
    }
    for digest in combined_outer_bindings.digests_v1() {
        binding.update(&digest);
    }
    binding.update(&(equation_verified.codec_offset as u32).to_be_bytes());
    let binding_digest = binding.finalize();
    if [residual_digest, binding_digest].contains(&[0; DIGEST_BYTES_V1]) {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidIntegrity);
    }
    Ok(VerifiedKernelV1 {
        residual: equation_verified.residual,
        manifest_digest: equation_verified.manifest_digest,
        source_context_digest: equation_verified.source_context_digest,
        point_root: equation_verified.point_root,
        replay_receipt_digest: equation_verified.replay_receipt_digest,
        pre_challenge_binding_digest: equation_verified.pre_challenge_binding_digest,
        tau_digest: equation_verified.tau_digest,
        q_digest: equation_verified.q_digest,
        proof_digest: equation_verified.proof_digest,
        residual_digest,
        binding_digest,
    })
}

/// Move-only evidence that the combined direct-plus-membership predecessor also
/// passed the exact source/packing same-opening child.
#[allow(
    missing_copy_implementations,
    reason = "the combined predecessor and downstream residual must advance exactly once"
)]
pub(super) struct RnsNativeSourcePackingSameOpeningPrerequisiteV1<
    'proof,
    P: RnsNativeSourcePackingCombinedDirectMembershipPredecessorV1<'proof>,
> {
    previous: P,
    residual: &'proof [u8],
    manifest_digest: [u8; DIGEST_BYTES_V1],
    source_context_digest: [u8; DIGEST_BYTES_V1],
    point_root: [u8; DIGEST_BYTES_V1],
    replay_receipt_digest: [u8; DIGEST_BYTES_V1],
    pre_challenge_binding_digest: [u8; DIGEST_BYTES_V1],
    tau_digest: [u8; DIGEST_BYTES_V1],
    q_digest: [u8; DIGEST_BYTES_V1],
    proof_digest: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    binding_digest: [u8; DIGEST_BYTES_V1],
}

impl<'proof, P: RnsNativeSourcePackingCombinedDirectMembershipPredecessorV1<'proof>>
    RnsNativeSourcePackingSameOpeningPrerequisiteV1<'proof, P>
{
    pub(super) const fn previous(&self) -> &P {
        &self.previous
    }

    pub(super) const fn residual(&self) -> &'proof [u8] {
        self.residual
    }

    pub(super) const fn point_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.point_root
    }

    pub(super) const fn pre_challenge_binding_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.pre_challenge_binding_digest
    }

    pub(super) const fn tau_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.tau_digest
    }

    pub(super) const fn proof_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.proof_digest
    }

    pub(super) const fn residual_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.residual_digest
    }

    pub(super) const fn binding_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.binding_digest
    }
}

/// Consume future combined direct-plus-membership evidence into this child.
///
/// No production `P` exists.  The function is present solely to freeze the
/// eventual insertion point and move-only ownership shape while its production
/// predecessor remains unavailable.
pub(super) fn verify_rns_native_source_packing_same_opening_v1<'proof, P, R>(
    previous: P,
    context: RnsNativeSourcePackingSameOpeningContextV1,
    replay_source: R,
) -> Result<
    RnsNativeSourcePackingSameOpeningPrerequisiteV1<'proof, P>,
    RnsNativeSourcePackingSameOpeningErrorV1,
>
where
    P: RnsNativeSourcePackingCombinedDirectMembershipPredecessorV1<'proof>,
    R: RnsNativeSourcePackingAggregateReplayV1,
{
    // Obtain the already-verified combined successor first.  Its typed safe
    // core is successor-independent and must match before the one-shot source
    // is touched.  Its enclosing/combined outer bundle is deliberately
    // unavailable until after the child equation returns an equation-verified
    // state.
    let wire: &'proof [u8] = previous.same_opening_successor_v1();
    if context.safe_core != previous.successor_independent_safe_core_v1() {
        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext);
    }
    let equation_verified = verify_equation_kernel_v1(
        context,
        replay_source,
        wire,
        FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1,
    )?;
    let combined_outer_bindings = previous.combined_outer_bindings_v1();
    let verified = finalize_verified_kernel_v1(equation_verified, combined_outer_bindings)?;
    Ok(RnsNativeSourcePackingSameOpeningPrerequisiteV1 {
        previous,
        residual: verified.residual,
        manifest_digest: verified.manifest_digest,
        source_context_digest: verified.source_context_digest,
        point_root: verified.point_root,
        replay_receipt_digest: verified.replay_receipt_digest,
        pre_challenge_binding_digest: verified.pre_challenge_binding_digest,
        tau_digest: verified.tau_digest,
        q_digest: verified.q_digest,
        proof_digest: verified.proof_digest,
        residual_digest: verified.residual_digest,
        binding_digest: verified.binding_digest,
    })
}

#[cfg(test)]
#[path = "rns_native_source_packing_same_opening_tests.rs"]
mod tests;
