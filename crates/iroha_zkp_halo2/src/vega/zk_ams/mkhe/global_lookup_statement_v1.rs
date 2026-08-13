//! Static global-lookup statement prerequisite for the Phase-23 RNS link.
//!
//! This private module freezes only topology, statement roles, transcript
//! ordering, opaque streaming-owner contracts, and conditional accounting.  It
//! does not construct or verify a proof.  Its production seals are uninhabited,
//! and every proof, binding, receipt, authority, RSS, and release gate remains
//! false.

#![allow(dead_code, reason = "production entry seals are uninhabited")]

use core::convert::Infallible;

use crate::vega::{VEGA_T256_SCALAR_MODULUS_BE_V1, sponge::Keccak256};

#[path = "global_lookup_statement_v1/challenge_v1.rs"]
mod challenge_v1;
#[path = "global_lookup_statement_v1/external_sumcheck_storage_v1.rs"]
mod external_sumcheck_storage_v1;
#[path = "global_lookup_statement_v1/global_lookup_committed_mle_v1.rs"]
mod global_lookup_committed_mle_v1;

use challenge_v1::challenge_manifest_digest_v1;

const GLOBAL_LOOKUP_VERSION_V1: u8 = 1;
const COORDINATES_PER_PLANE_V1: usize = 1 << 14;
const EXISTING_ACTIVE_PLANES_V1: usize = 11_696;
const COMPARATOR_GROUPS_V1: usize = 344;
const COMPARATOR_PLANES_PER_GROUP_V1: usize = 53;
const COMPARATOR_PLANES_V1: usize = 18_232;
const SMALL_SOURCE_BLOCKS_V1: usize = 1_032;
const SMALL_SOURCE_PLANES_PER_BLOCK_V1: usize = 4;
const SMALL_SOURCE_PLANES_V1: usize = 4_128;
const Q_MASK_BLOCKS_V1: usize = 1_520;
const Q_MASK_PLANES_PER_BLOCK_V1: usize = 16;
const Q_MASK_PLANES_V1: usize = 24_320;
const ADDED_PLANES_V1: usize = 46_680;
const ADDED_PRE_Z_PLANES_V1: usize = 26_608;
const ADDED_INVERSE_PLANES_V1: usize = 20_072;
const ADDED_ACTIVE_PLANES_V1: usize = 20_072;
const ACTIVE_LOOKUP_PLANES_V1: usize = 31_768;
const VIRTUAL_ZERO_PLANES_V1: usize = 1_000;
const PADDED_LOOKUP_PLANES_V1: usize = 1 << 15;
const ACTIVE_LOOKUP_VALUES_V1: u64 = 520_486_912;

const COEFFICIENT_EQUATIONS_V1: usize = 14;
const PRIOR_CUBIC_MESSAGES_V1: usize = 233;
const REQUIRED_CUBIC_MESSAGES_V1: usize = 234;
const CUBIC_MESSAGE_BYTES_V1: usize = 96;
const HIDDEN_ENDPOINTS_V1: usize = 52;
const MULTIPLICITY_COMMITMENTS_V1: usize = 1;
const SUMCHECK_MASK_COMMITMENTS_V1: usize = 1;
const COEFFICIENT_IPAS_V1: usize = 16;
const TABLE_IPAS_V1: usize = 1;
const MASK_IPAS_V1: usize = 1;
const ENDPOINT_GATES_PER_STATEMENT_V1: usize = 2;
const ENDPOINT_STATEMENTS_V1: usize = 16;
const ENDPOINT_GATES_V1: usize = 32;
const POST_BATCH_RESIDUAL_STATEMENTS_V1: [usize; 3] = [3, 5, 8];
const REQUIRED_POST_BATCH_RESIDUAL_COMMITMENTS_V1: usize = 3;
const REQUIRED_VECTOR_ARITHMETIC_PROOFS_V1: usize = 3;
const POST_BATCH_RESIDUAL_VECTOR_LENGTH_V1: usize = COORDINATES_PER_PLANE_V1;
const POST_BATCH_RESIDUAL_COMMITMENT_BYTES_V1: usize = 3 * 33;
const POST_BATCH_RESIDUAL_COMMITMENT_FRAMES_INSTANTIATED_V1: bool = true;
const VECTOR_ARITHMETIC_PROOFS_INSTANTIATED_V1: bool = false;
const COEFFICIENT_CHALLENGE_WIRE_BYTES_V1: usize = 0;

const PRIOR_RADIX_QPCS_BYTES_V1: usize = 31_395_509;
const CONDITIONAL_CROSS_FIELD_BYTES_V1: usize = 1_828_422;
const CUBIC_EXTENSION_BYTES_V1: usize = CUBIC_MESSAGE_BYTES_V1;
const COEFFICIENT_IPA_CORRECTION_BYTES_V1: usize = 16 * (1_381 - 1_022);
const TABLE_IPA_CORRECTION_BYTES_V1: usize = 1_447 - 1_088;
const NEW_MASK_COMMITMENT_AND_IPA_BYTES_V1: usize = 33 + 1_117;
const MASK_IPA_CORRECTION_BYTES_V1: usize = NEW_MASK_COMMITMENT_AND_IPA_BYTES_V1 - 725;
const GLOBAL_LOOKUP_DELTA_BYTES_V1: usize = CUBIC_EXTENSION_BYTES_V1
    + COEFFICIENT_IPA_CORRECTION_BYTES_V1
    + TABLE_IPA_CORRECTION_BYTES_V1
    + MASK_IPA_CORRECTION_BYTES_V1;
const KNOWN_WIRE_LOWER_BOUND_BEFORE_VECTOR_ARITHMETIC_PROOFS_V1: usize =
    33_230_555 + POST_BATCH_RESIDUAL_COMMITMENT_BYTES_V1;
const CONDITIONAL_CAP_BYTES_V1: usize = 32 * 1_048_576;
const CONDITIONAL_TOTAL_BYTES_V1: Option<usize> = None;
const CONDITIONAL_MARGIN_BYTES_V1: Option<usize> = None;
const LOOKUP_SOUNDNESS_NUMERATOR_V1: u64 = 520_519_678;
const LOOKUP_SOUNDNESS_BITS_X100_FLOOR_V1: u32 = 22_704;

const TOPOLOGY_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.global-lookup.topology\0";
const TRANSCRIPT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.global-lookup.transcript\0";
const SOUNDNESS_FORMULA_V1: &[u8] =
    b"520519678/(pT-32768)<2^-227;lookup-rational-degree=520486912+32768-2";
const LOOKUP_INDEX_LANGUAGE_V1: &[u8] = b"D={0,1}^29;x=(c_0..c_13,y_0..y_14),little-endian,rounds-coordinate-then-plane;S(y)=sum_{p=0}^{31767}eq(bin15(p),y);s(c,y)=S(y);A(x)=MLE(active-candidates||1000-zero-planes);E0(c)=eq(0^14,c);T={0..32767};M(y)=MLE(m_0..m_32767);Q_z(y)=MLE_t((z-t)^-1)";
const LOOKUP_RELATION_LANGUAGE_V1: &[u8] = b"commit(A,M,mask)before-z;derive-z-notin-T;commit-U;derive-rho[29],alpha,lambda,mu-independent-nonzero;chi_rho(x)=prod_i((1-rho_i)(1-x_i)+rho_i*x_i);L=(z-A)*U-s;F=alpha*chi_rho*L+lambda*(U-E0*M*Q_z)+mu*(E0*M-s);sum_{x in D}F(x)=0;sum_t(m_t)=520486912 follows from mu term;padding-plane p>=31768:A=s=U=0 and is not a multiplicity occurrence";
const LOOKUP_MASK_LANGUAGE_V1: &[u8] = b"one-pre-z-mask-commitment=(a_j,b_j,c_j)for-global-messages-j=0..233:702-scalars-zero-padded-to-1024;segments=(14*s,14),s=0..13;(196,9);(205,29);carry-resets-at-each-segment;h=1/2;terminal-weight(s,t)=(h^(len-1-t)*(r_t^3-h),h^(len-1-t)*(r_t^2-h),h^(len-1-t)*(r_t-h));send-exact-96-byte-gtilde_j;derive-r_j-only-after-gtilde_j;xi-batches-statements-by-xi^s;padding-weights-702..1023=0";
const LOOKUP_ENDPOINT_LANGUAGE_V1: &[u8] = b"ordered-hidden-endpoints=(A*=A(r),U*=U(r),V*,M*=M(r_y),Z*=carry_29,R*=F(r));lookup-gate0:(z-A*)*U*=V*;lookup-gate1:R*-[alpha*chi_rho(r)*(V*-S(r_y))+lambda*(U*-E0(r_c)*M*Q_z(r_y))+mu*(E0(r_c)*M*-S(r_y))]=0;public-linear-constraint:R*+Z*-C15=0;mask-constraints:s=0..13:Z_s=<w_s,mask>,Cfinal_s-B_s-Z_s=0;s=14:<w_14,mask>+R_14-C14=0;s=15:<w_15,mask>-Z*=0;lookup-eval-batch:C_AU=sum_p(eq(r_y,bin15(p))*(C_A[p]+nu_15*C_U[p]));target=A*+nu_15*U*;open-n16384-at-r_c;one-purpose-bound-32-gate-scalar-BP-binds-all-52-endpoints-and-two-gates-per-16-statements";
const LOOKUP_SOUNDNESS_LANGUAGE_V1: &[u8] = b"m_t-are-pre-z-field-elements;nonnegative-range-is-not-required;sum-m-is-constrained;lookup-rational-numerator-degree<=520486912+32768-2=520519678;rho-and-independent-nonzero-alpha/lambda/mu-batch-local-inverse,log-identity,total;mask-IPA-prevents-free-Z-and-mask-telescoping-does-not-change-acceptance;sumcheck-batching-IPA-DL-union-theorem-remains-uninstantiated";
const COEFFICIENT_AGGREGATION_LANGUAGE_V1: &[u8] = b"for-s=0..13,for-v-in-{0,1}^14:q_s[v]=sum_o(kappa^o*sum_e(delta^e*R_{s,o,e}[v]))where-R[v]-is-the-frozen-residual-evaluated-at-Boolean-witness-coordinate-v;owner-o-order=canonical-topology-order;residual-e-order=left-to-right-formula-order;Q_s(x)=MLE(q_s)(x);for-linear-families-s-notin(3,5,8)-this-equals-the-direct-linear-residual-extension;F_s(x)=eq(tau,x)*Q_s(x);sum_{v-in-{0,1}^14}F_s(v)-initial-claim_s=0;initial-claim_s=0;unmasked-round-polynomial-per-variable-degree<=2-and-cubic-coefficient=0;existing-CompressedUnivariate-degree3-envelope-wire=(constant,quadratic,cubic)-canonical-le-96B;linear=claim-2*constant-quadratic-cubic;precommitted-zero-sum-cubic-mask-preserves-the-Boolean-telescope-and-degree<=3-and-may-make-the-transmitted-cubic-coordinate-nonzero";
const SPECIAL_QUADRATIC_AGGREGATION_LANGUAGE_V1: &[u8] = b"on-v-in-{0,1}^14:q_3[v]=sum_g(kappa^g*(bD_g[v]*(bD_g[v]-1)+delta*bS_g[v]*(bS_g[v]-1)+delta^2*bD_g[v]*bS_g[v]));q_5[v]=sum_g(kappa^g*(sum_h=0..17(delta^h*beta_g,h[v]*(beta_g,h[v]-1))+delta^18*(m_g[v]-bD_g[v]*beta_g,16[v])+delta^19*(beta_g,17[v]-beta_g,16[v]+m_g[v])));q_8[v]=sum_u(kappa^u*(x_u[v]+n_u[v])*n_u[v]);Q_3=MLE(q_3);Q_5=MLE(q_5);Q_8=MLE(q_8);never-extend-these-products-off-the-Boolean-cube";
const COEFFICIENT_ENDPOINT_LANGUAGE_V1: &[u8] = b"for-s=0..13:ordered-hidden-endpoints=(A_s=Q_s(r_s),B_s=eq(tau,r_s)*A_s,Z_s=mask-terminal_s);A_s-opens-the-same-framed-q_s-commitment-for-s-in(3,5,8)-or-the-same-verifier-derived-linear-aggregate-for-all-other-s-that-the-sumcheck-uses;gate0:B_s-eq(tau,r_s)*A_s=0;gate1:Cfinal_s-B_s-Z_s=0";
const POST_BATCH_RESIDUAL_REQUIREMENT_LANGUAGE_V1: &[u8] = b"after-kappa-delta-before-gtilde0:ordered-statements=(3,5,8);each-requires-one-blinded-length-2^14-q_s-vector-commitment-and-one-vector-arithmetic-proof-binding-every-q_s[v]-to-its-exact-frozen-Boolean-coordinate-formula-in-canonical-owner/residual-order;commitments=3*canonical-nonidentity-33B=99B-and-transcript-frames-instantiated;vector-proofs=3-required-but-codec-uninstantiated-and-wire-bytes-undefined;not-counted-as-hidden-endpoints-or-IPAs";

const LOOKUP_PROOF_VERIFIED_V1: bool = false;
const ZERO_KNOWLEDGE_ACCEPTED_V1: bool = false;
const SOURCE_SAME_OPENING_VERIFIED_V1: bool = false;
const PACKING_SAME_OPENING_VERIFIED_V1: bool = false;
const CROSS_FIELD_BINDING_VERIFIED_V1: bool = false;
const STREAMING_OWNERS_WIRED_V1: bool = false;
const COMPLETE_ACCOUNTING_QUALIFIED_V1: bool = false;
const OPERATIONAL_RECEIPT_ACCEPTED_V1: bool = false;
const AUTHORITY_MINTED_V1: bool = false;
const RSS_QUALIFIED_V1: bool = false;
const RELEASE_READY_V1: bool = false;

const _: () = {
    assert!(ADDED_PLANES_V1 == COMPARATOR_PLANES_V1 + SMALL_SOURCE_PLANES_V1 + Q_MASK_PLANES_V1);
    assert!(ADDED_PLANES_V1 == ADDED_PRE_Z_PLANES_V1 + ADDED_INVERSE_PLANES_V1);
    assert!(ADDED_ACTIVE_PLANES_V1 == 5_848 + 2_064 + 12_160);
    assert!(ACTIVE_LOOKUP_PLANES_V1 == EXISTING_ACTIVE_PLANES_V1 + ADDED_ACTIVE_PLANES_V1);
    assert!(PADDED_LOOKUP_PLANES_V1 == ACTIVE_LOOKUP_PLANES_V1 + VIRTUAL_ZERO_PLANES_V1);
    assert!(ACTIVE_LOOKUP_VALUES_V1 == 520_486_912);
    assert!(HIDDEN_ENDPOINTS_V1 == COEFFICIENT_EQUATIONS_V1 * 3 + 4 + 6);
    assert!(MULTIPLICITY_COMMITMENTS_V1 == 1 && SUMCHECK_MASK_COMMITMENTS_V1 == 1);
    assert!(COEFFICIENT_IPAS_V1 == COEFFICIENT_EQUATIONS_V1 + 1 + 1);
    assert!(TABLE_IPAS_V1 == 1 && MASK_IPAS_V1 == 1);
    assert!(ENDPOINT_GATES_V1 == ENDPOINT_STATEMENTS_V1 * ENDPOINT_GATES_PER_STATEMENT_V1);
    assert!(
        KNOWN_WIRE_LOWER_BOUND_BEFORE_VECTOR_ARITHMETIC_PROOFS_V1
            == PRIOR_RADIX_QPCS_BYTES_V1
                + CONDITIONAL_CROSS_FIELD_BYTES_V1
                + GLOBAL_LOOKUP_DELTA_BYTES_V1
                + POST_BATCH_RESIDUAL_COMMITMENT_BYTES_V1
    );
    assert!(CONDITIONAL_TOTAL_BYTES_V1.is_none() && CONDITIONAL_MARGIN_BYTES_V1.is_none());
    assert!(GLOBAL_LOOKUP_DELTA_BYTES_V1 == 6_624);
    assert!(COEFFICIENT_CHALLENGE_WIRE_BYTES_V1 == 0);
    assert!(REQUIRED_POST_BATCH_RESIDUAL_COMMITMENTS_V1 == POST_BATCH_RESIDUAL_STATEMENTS_V1.len());
    assert!(REQUIRED_VECTOR_ARITHMETIC_PROOFS_V1 == POST_BATCH_RESIDUAL_STATEMENTS_V1.len());
    assert!(POST_BATCH_RESIDUAL_VECTOR_LENGTH_V1 == 1 << 14);
    assert!(POST_BATCH_RESIDUAL_COMMITMENT_FRAMES_INSTANTIATED_V1);
    assert!(!VECTOR_ARITHMETIC_PROOFS_INSTANTIATED_V1);
    assert!((LOOKUP_SOUNDNESS_NUMERATOR_V1 << 35) < 0xffff_ffff_0000_0001);
    assert!(!RELEASE_READY_V1);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GlobalLookupErrorV1 {
    Shape,
    Order,
    Context,
    Arithmetic,
    Encoding,
    ChallengeExhausted,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AddedPlaneRoleV1 {
    ComparatorDifferenceDigit = 1,
    ComparatorMixedTop = 2,
    ComparatorBorrow = 3,
    ComparatorDifferenceInverse = 4,
    SmallSigned = 5,
    SmallNegativeMagnitude = 6,
    SmallPositiveInverse = 7,
    SmallNegativeInverse = 8,
    QMaskDigit = 9,
    QMaskDigitInverse = 10,
    QMaskComplementDigit = 11,
    QMaskComplementInverse = 12,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AddedPlaneCoordinateV1 {
    ordinal: usize,
    role: AddedPlaneRoleV1,
    active_role: Option<ActiveLookupRoleV1>,
    owner: usize,
    column: usize,
    active_lookup_slot: Option<usize>,
}

#[rustfmt::skip]
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActiveLookupRoleV1 { ComparatorDifferenceDigit = 1, SmallPositive = 2, SmallNegativeMagnitude = 3, QMaskDigit = 4, QMaskComplementDigit = 5 }

impl AddedPlaneCoordinateV1 {
    const fn is_inverse_v1(self) -> bool {
        matches!(
            self.role,
            AddedPlaneRoleV1::ComparatorDifferenceInverse
                | AddedPlaneRoleV1::SmallPositiveInverse
                | AddedPlaneRoleV1::SmallNegativeInverse
                | AddedPlaneRoleV1::QMaskDigitInverse
                | AddedPlaneRoleV1::QMaskComplementInverse
        )
    }
}

fn added_plane_coordinate_v1(
    ordinal: usize,
) -> Result<AddedPlaneCoordinateV1, GlobalLookupErrorV1> {
    if ordinal < COMPARATOR_PLANES_V1 {
        let owner = ordinal / COMPARATOR_PLANES_PER_GROUP_V1;
        let local = ordinal % COMPARATOR_PLANES_PER_GROUP_V1;
        let (role, active_role, column, active) = match local {
            0..=16 => (
                AddedPlaneRoleV1::ComparatorDifferenceDigit,
                Some(ActiveLookupRoleV1::ComparatorDifferenceDigit),
                local,
                Some(EXISTING_ACTIVE_PLANES_V1 + owner * 17 + local),
            ),
            17 => (AddedPlaneRoleV1::ComparatorMixedTop, None, 0, None),
            18..=35 => (AddedPlaneRoleV1::ComparatorBorrow, None, local - 18, None),
            36..=52 => (
                AddedPlaneRoleV1::ComparatorDifferenceInverse,
                None,
                local - 36,
                None,
            ),
            _ => return Err(GlobalLookupErrorV1::Shape),
        };
        return Ok(AddedPlaneCoordinateV1 {
            ordinal,
            role,
            active_role,
            owner,
            column,
            active_lookup_slot: active,
        });
    }
    let small_ordinal = ordinal - COMPARATOR_PLANES_V1;
    if small_ordinal < SMALL_SOURCE_PLANES_V1 {
        let owner = small_ordinal / SMALL_SOURCE_PLANES_PER_BLOCK_V1;
        let local = small_ordinal % SMALL_SOURCE_PLANES_PER_BLOCK_V1;
        let role = [
            AddedPlaneRoleV1::SmallSigned,
            AddedPlaneRoleV1::SmallNegativeMagnitude,
            AddedPlaneRoleV1::SmallPositiveInverse,
            AddedPlaneRoleV1::SmallNegativeInverse,
        ][local];
        let active_role = [
            Some(ActiveLookupRoleV1::SmallPositive),
            Some(ActiveLookupRoleV1::SmallNegativeMagnitude),
            None,
            None,
        ][local];
        let active_lookup_slot = (local < 2)
            .then_some(EXISTING_ACTIVE_PLANES_V1 + 5_848 + local * SMALL_SOURCE_BLOCKS_V1 + owner);
        return Ok(AddedPlaneCoordinateV1 {
            ordinal,
            role,
            active_role,
            owner,
            column: 0,
            active_lookup_slot,
        });
    }
    let q_ordinal = small_ordinal - SMALL_SOURCE_PLANES_V1;
    if q_ordinal >= Q_MASK_PLANES_V1 {
        return Err(GlobalLookupErrorV1::Shape);
    }
    let owner = q_ordinal / Q_MASK_PLANES_PER_BLOCK_V1;
    let local = q_ordinal % Q_MASK_PLANES_PER_BLOCK_V1;
    let (role, active_role, column, active_column) = match local {
        0..=3 => (
            AddedPlaneRoleV1::QMaskDigit,
            Some(ActiveLookupRoleV1::QMaskDigit),
            local,
            Some(local),
        ),
        4..=7 => (AddedPlaneRoleV1::QMaskDigitInverse, None, local - 4, None),
        8..=11 => (
            AddedPlaneRoleV1::QMaskComplementDigit,
            Some(ActiveLookupRoleV1::QMaskComplementDigit),
            local - 8,
            Some(local - 4),
        ),
        12..=15 => (
            AddedPlaneRoleV1::QMaskComplementInverse,
            None,
            local - 12,
            None,
        ),
        _ => return Err(GlobalLookupErrorV1::Shape),
    };
    Ok(AddedPlaneCoordinateV1 {
        ordinal,
        role,
        active_role,
        owner,
        column,
        active_lookup_slot: active_column.map(|column| {
            EXISTING_ACTIVE_PLANES_V1 + 5_848 + 2_064 + column * Q_MASK_BLOCKS_V1 + owner
        }),
    })
}

#[rustfmt::skip]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LookupPlaneCoordinateV1 { Existing { ordinal: usize, role: ExistingLookupPlaneRoleV1 }, Added(AddedPlaneCoordinateV1), VirtualZero { ordinal: usize } }

#[rustfmt::skip]
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExistingLookupPlaneRoleV1 { DDigit = 1, SlackDigit = 2 }

#[rustfmt::skip]
const fn existing_lookup_role_v1(slot: usize) -> ExistingLookupPlaneRoleV1 { if slot < 5_848 { ExistingLookupPlaneRoleV1::DDigit } else { ExistingLookupPlaneRoleV1::SlackDigit } }

fn lookup_plane_coordinate_v1(slot: usize) -> Result<LookupPlaneCoordinateV1, GlobalLookupErrorV1> {
    if slot < EXISTING_ACTIVE_PLANES_V1 {
        return Ok(LookupPlaneCoordinateV1::Existing {
            ordinal: slot,
            role: existing_lookup_role_v1(slot),
        });
    }
    if slot < ACTIVE_LOOKUP_PLANES_V1 {
        let added = slot - EXISTING_ACTIVE_PLANES_V1;
        let ordinal = if added < 5_848 {
            (added / 17) * COMPARATOR_PLANES_PER_GROUP_V1 + added % 17
        } else if added < 5_848 + 2_064 {
            let local = added - 5_848;
            COMPARATOR_PLANES_V1
                + (local % SMALL_SOURCE_BLOCKS_V1) * SMALL_SOURCE_PLANES_PER_BLOCK_V1
                + local / SMALL_SOURCE_BLOCKS_V1
        } else {
            let local = added - 5_848 - 2_064;
            let role = local / Q_MASK_BLOCKS_V1;
            COMPARATOR_PLANES_V1
                + SMALL_SOURCE_PLANES_V1
                + (local % Q_MASK_BLOCKS_V1) * Q_MASK_PLANES_PER_BLOCK_V1
                + if role < 4 { role } else { role + 4 }
        };
        let coordinate = added_plane_coordinate_v1(ordinal)?;
        if coordinate.active_lookup_slot != Some(slot) {
            return Err(GlobalLookupErrorV1::Shape);
        }
        return Ok(LookupPlaneCoordinateV1::Added(coordinate));
    }
    if slot < PADDED_LOOKUP_PLANES_V1 {
        return Ok(LookupPlaneCoordinateV1::VirtualZero {
            ordinal: slot - ACTIVE_LOOKUP_PLANES_V1,
        });
    }
    Err(GlobalLookupErrorV1::Shape)
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoefficientEquationRoleV1 {
    DRadixReconstruction = 1,
    SlackRadixReconstruction = 2,
    CanonicalComplement = 3,
    BatchedTopBooleanAndExclusion = 4,
    CenteringComparatorSubtraction = 5,
    ComparatorBorrowBooleanity = 6,
    CenteredLiftSelector = 7,
    SmallPositiveLinearDerivation = 8,
    SmallSignDisjointness = 9,
    QMaskRadixReconstruction = 10,
    QMaskComplementReconstruction = 11,
    StructuralMaskTopZero = 12,
    SourceCoefficientSameOpening = 13,
    PackingTransposeSameOpening = 14,
}

#[rustfmt::skip]
const COEFFICIENT_EQUATION_ROLES_V1: [CoefficientEquationRoleV1; COEFFICIENT_EQUATIONS_V1] = [
    CoefficientEquationRoleV1::DRadixReconstruction, CoefficientEquationRoleV1::SlackRadixReconstruction,
    CoefficientEquationRoleV1::CanonicalComplement, CoefficientEquationRoleV1::BatchedTopBooleanAndExclusion,
    CoefficientEquationRoleV1::CenteringComparatorSubtraction, CoefficientEquationRoleV1::ComparatorBorrowBooleanity,
    CoefficientEquationRoleV1::CenteredLiftSelector, CoefficientEquationRoleV1::SmallPositiveLinearDerivation,
    CoefficientEquationRoleV1::SmallSignDisjointness, CoefficientEquationRoleV1::QMaskRadixReconstruction,
    CoefficientEquationRoleV1::QMaskComplementReconstruction, CoefficientEquationRoleV1::StructuralMaskTopZero,
    CoefficientEquationRoleV1::SourceCoefficientSameOpening, CoefficientEquationRoleV1::PackingTransposeSameOpening,
];

impl CoefficientEquationRoleV1 {
    const fn formula_v1(self) -> &'static [u8] {
        match self {
            Self::DRadixReconstruction => b"D=sum_{h=0}^{16}B^h*d_h+B^17*b_D",
            Self::SlackRadixReconstruction => b"S=sum_{h=0}^{16}B^h*s_h+B^17*b_S",
            Self::CanonicalComplement => b"D+S=pT-1",
            Self::BatchedTopBooleanAndExclusion => b"for-g=0..343:residuals=(bD_g*(bD_g-1),bS_g*(bS_g-1),bD_g*bS_g)",
            Self::CenteringComparatorSubtraction => b"B=2^15;center=(pT-1)/2;K=center+1;K_17=0;h=0:D_0-K_0=Delta_0-B*beta_0;for-h=1..16:D_h-K_h-beta_{h-1}=Delta_h-B*beta_h",
            Self::ComparatorBorrowBooleanity => b"for-g=0..343:for-h=0..17:beta_g,h*(beta_g,h-1)=0;m_g=bD_g*beta_g,16;beta_g,17=beta_g,16-m_g;beta_g,17=1 iff D_g<K",
            Self::CenteredLiftSelector => b"sigma=1-beta_17;M=D-sigma*pT",
            Self::SmallPositiveLinearDerivation => b"x_plus=x_signed+x_negative",
            Self::SmallSignDisjointness => b"for-u=0..1031:(x_u+n_u)*n_u=0",
            Self::QMaskRadixReconstruction => b"S_q=sum_{h=0}^{3}B^h*s_qh",
            Self::QMaskComplementReconstruction => b"S_q+S_q_bar=q-1",
            Self::StructuralMaskTopZero => b"S_q[N-1]=0",
            Self::SourceCoefficientSameOpening => b"opened_source[record,block,i]=source_owner[record,block,i]",
            Self::PackingTransposeSameOpening => b"packed[(record*8+group)*16384+i*64+block]=source[((record*8+group)*64+block)*256+i]",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EquationEndpointV1 {
    AggregateAtPoint,
    EqualityWeightedAggregate,
    MaskTerminal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GroupBinderEndpointV1 {
    Source,
    Packed,
    Selector,
    Residual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LookupEndpointV1 {
    Candidate,
    Inverse,
    InverseProduct,
    Multiplicity,
    MaskedAccumulator,
    Residual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HiddenEndpointRoleV1 {
    Equation {
        equation: CoefficientEquationRoleV1,
        endpoint: EquationEndpointV1,
    },
    GroupBinder(GroupBinderEndpointV1),
    GlobalLookup(LookupEndpointV1),
}

fn hidden_endpoint_role_v1(ordinal: usize) -> Result<HiddenEndpointRoleV1, GlobalLookupErrorV1> {
    if ordinal < COEFFICIENT_EQUATIONS_V1 * 3 {
        let endpoint = match ordinal % 3 {
            0 => EquationEndpointV1::AggregateAtPoint,
            1 => EquationEndpointV1::EqualityWeightedAggregate,
            2 => EquationEndpointV1::MaskTerminal,
            _ => return Err(GlobalLookupErrorV1::Shape),
        };
        return Ok(HiddenEndpointRoleV1::Equation {
            equation: COEFFICIENT_EQUATION_ROLES_V1[ordinal / 3],
            endpoint,
        });
    }
    let tail = ordinal - COEFFICIENT_EQUATIONS_V1 * 3;
    if tail < 4 {
        return Ok(HiddenEndpointRoleV1::GroupBinder(
            [
                GroupBinderEndpointV1::Source,
                GroupBinderEndpointV1::Packed,
                GroupBinderEndpointV1::Selector,
                GroupBinderEndpointV1::Residual,
            ][tail],
        ));
    }
    if tail < 10 {
        return Ok(HiddenEndpointRoleV1::GlobalLookup(
            [
                LookupEndpointV1::Candidate,
                LookupEndpointV1::Inverse,
                LookupEndpointV1::InverseProduct,
                LookupEndpointV1::Multiplicity,
                LookupEndpointV1::MaskedAccumulator,
                LookupEndpointV1::Residual,
            ][tail - 4],
        ));
    }
    Err(GlobalLookupErrorV1::Shape)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IpaStatementRoleV1 {
    Equation(CoefficientEquationRoleV1),
    GroupBinder,
    GlobalLookup,
}

fn ipa_statement_role_v1(ordinal: usize) -> Result<IpaStatementRoleV1, GlobalLookupErrorV1> {
    match ordinal {
        0..=13 => Ok(IpaStatementRoleV1::Equation(
            COEFFICIENT_EQUATION_ROLES_V1[ordinal],
        )),
        14 => Ok(IpaStatementRoleV1::GroupBinder),
        15 => Ok(IpaStatementRoleV1::GlobalLookup),
        _ => Err(GlobalLookupErrorV1::Shape),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EndpointGateRoleV1 {
    StatementGate0,
    StatementGate1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EndpointGateCoordinateV1 {
    statement_ordinal: usize,
    role: EndpointGateRoleV1,
}

fn endpoint_gate_coordinate_v1(
    ordinal: usize,
) -> Result<EndpointGateCoordinateV1, GlobalLookupErrorV1> {
    if ordinal >= ENDPOINT_GATES_V1 {
        return Err(GlobalLookupErrorV1::Shape);
    }
    Ok(EndpointGateCoordinateV1 {
        statement_ordinal: ordinal / 2,
        role: if ordinal % 2 == 0 {
            EndpointGateRoleV1::StatementGate0
        } else {
            EndpointGateRoleV1::StatementGate1
        },
    })
}

#[rustfmt::skip]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CubicMessageCoordinateV1 { ordinal: usize, role: CubicMessageRoleV1, equation: Option<CoefficientEquationRoleV1>, local_round: usize, extends_prior_schedule: bool }

#[rustfmt::skip]
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CubicMessageRoleV1 { Equation = 1, GroupBinder = 2, GlobalLookup = 3 }

#[rustfmt::skip]
fn cubic_message_coordinate_v1(ordinal: usize) -> Result<CubicMessageCoordinateV1, GlobalLookupErrorV1> {
    let (role, equation, local_round) = match ordinal {
        0..196 => (CubicMessageRoleV1::Equation, Some(COEFFICIENT_EQUATION_ROLES_V1[ordinal / 14]), ordinal % 14),
        196..205 => (CubicMessageRoleV1::GroupBinder, None, ordinal - 196),
        205..234 => (CubicMessageRoleV1::GlobalLookup, None, ordinal - 205),
        _ => return Err(GlobalLookupErrorV1::Shape),
    };
    Ok(CubicMessageCoordinateV1 { ordinal, role, equation, local_round, extends_prior_schedule: ordinal == PRIOR_CUBIC_MESSAGES_V1 })
}

fn absorb_usize_v1(hash: &mut Keccak256, value: usize) {
    hash.update(&(value as u64).to_be_bytes());
}

#[rustfmt::skip]
/// Return the exact topology digest shared with authenticated source replay.
pub(super) fn global_lookup_topology_digest_v1() -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(TOPOLOGY_DOMAIN_V1);
    hash.update(&[GLOBAL_LOOKUP_VERSION_V1]);
    for value in [
        COORDINATES_PER_PLANE_V1,
        EXISTING_ACTIVE_PLANES_V1,
        ADDED_PLANES_V1,
        ACTIVE_LOOKUP_PLANES_V1,
        VIRTUAL_ZERO_PLANES_V1,
        PADDED_LOOKUP_PLANES_V1,
        REQUIRED_CUBIC_MESSAGES_V1,
        HIDDEN_ENDPOINTS_V1,
        MULTIPLICITY_COMMITMENTS_V1,
        SUMCHECK_MASK_COMMITMENTS_V1,
        COEFFICIENT_IPAS_V1,
        TABLE_IPAS_V1,
        MASK_IPAS_V1,
        ENDPOINT_GATES_V1,
    ] {
        absorb_usize_v1(&mut hash, value);
    }
    for ordinal in 0..REQUIRED_CUBIC_MESSAGES_V1 { let message = cubic_message_coordinate_v1(ordinal).expect("constant cubic schedule"); hash.update(&[message.role as u8, message.equation.map(|role| role as u8).unwrap_or(0)]); absorb_usize_v1(&mut hash, message.ordinal); absorb_usize_v1(&mut hash, message.local_round); hash.update(&[u8::from(message.extends_prior_schedule)]); }
    for slot in 0..EXISTING_ACTIVE_PLANES_V1 {
        hash.update(&[existing_lookup_role_v1(slot) as u8]);
        absorb_usize_v1(&mut hash, slot);
    }
    for ordinal in 0..ADDED_PLANES_V1 {
        let coordinate = added_plane_coordinate_v1(ordinal).expect("constant topology");
        hash.update(&[coordinate.role as u8]);
        hash.update(&[coordinate.active_role.map(|role| role as u8).unwrap_or(0)]);
        absorb_usize_v1(&mut hash, coordinate.ordinal);
        absorb_usize_v1(&mut hash, coordinate.owner);
        absorb_usize_v1(&mut hash, coordinate.column);
        hash.update(
            &coordinate
                .active_lookup_slot
                .map(|slot| slot as u64)
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
    }
    hash.update(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    for role in COEFFICIENT_EQUATION_ROLES_V1 {
        hash.update(&[role as u8]);
        let formula = role.formula_v1();
        absorb_usize_v1(&mut hash, formula.len());
        hash.update(formula);
    }
    for formula in [
        LOOKUP_INDEX_LANGUAGE_V1,
        LOOKUP_RELATION_LANGUAGE_V1,
        LOOKUP_MASK_LANGUAGE_V1,
        LOOKUP_ENDPOINT_LANGUAGE_V1,
        LOOKUP_SOUNDNESS_LANGUAGE_V1,
        COEFFICIENT_AGGREGATION_LANGUAGE_V1,
        SPECIAL_QUADRATIC_AGGREGATION_LANGUAGE_V1,
        COEFFICIENT_ENDPOINT_LANGUAGE_V1,
        POST_BATCH_RESIDUAL_REQUIREMENT_LANGUAGE_V1,
    ] {
        absorb_usize_v1(&mut hash, formula.len());
        hash.update(formula);
    }
    hash.update(&challenge_manifest_digest_v1());
    for ordinal in 0..HIDDEN_ENDPOINTS_V1 {
        let role = hidden_endpoint_role_v1(ordinal).expect("constant endpoints");
        hash.update(&[endpoint_tag_v1(role)]);
    }
    for ordinal in 0..COEFFICIENT_IPAS_V1 {
        hash.update(&[ipa_tag_v1(
            ipa_statement_role_v1(ordinal).expect("constant IPA schedule"),
        )]);
    }
    for ordinal in 0..ENDPOINT_GATES_V1 {
        let gate = endpoint_gate_coordinate_v1(ordinal).expect("constant gate schedule");
        absorb_usize_v1(&mut hash, gate.statement_ordinal);
        hash.update(&[gate.role as u8]);
    }
    hash.update(SOUNDNESS_FORMULA_V1);
    hash.finalize()
}

const fn endpoint_tag_v1(role: HiddenEndpointRoleV1) -> u8 {
    match role {
        HiddenEndpointRoleV1::Equation { equation, endpoint } => {
            3 * (equation as u8 - 1) + endpoint as u8
        }
        HiddenEndpointRoleV1::GroupBinder(endpoint) => 42 + endpoint as u8,
        HiddenEndpointRoleV1::GlobalLookup(endpoint) => 46 + endpoint as u8,
    }
}

const fn ipa_tag_v1(role: IpaStatementRoleV1) -> u8 {
    match role {
        IpaStatementRoleV1::Equation(equation) => equation as u8,
        IpaStatementRoleV1::GroupBinder => 15,
        IpaStatementRoleV1::GlobalLookup => 16,
    }
}

fn chain_frame_v1(state: [u8; 32], label: &[u8], payload: &[u8]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(TRANSCRIPT_DOMAIN_V1);
    hash.update(&[GLOBAL_LOOKUP_VERSION_V1]);
    hash.update(&state);
    hash.update(&(label.len() as u16).to_be_bytes());
    hash.update(label);
    hash.update(&(payload.len() as u16).to_be_bytes());
    hash.update(payload);
    hash.finalize()
}

fn require_nonzero_v1(digest: [u8; 32]) -> Result<[u8; 32], GlobalLookupErrorV1> {
    (digest != [0; 32])
        .then_some(digest)
        .ok_or(GlobalLookupErrorV1::Context)
}

#[derive(Clone, Copy)]
struct GlobalLookupContextV1 {
    fixed_axes_digest: [u8; 32],
    source_binding_digest: [u8; 32],
    radix_range_digest: [u8; 32],
    packing_digest: [u8; 32],
    cross_field_digest: [u8; 32],
    qpcs_initial_root: [u8; 32],
}

/// Owners stream into opaque sinks; none of these contracts returns backing
/// storage, a point slice, or an owner handle.
trait OpaqueStreamingOwnerV1<Stage>: Sized {
    fn stream_into_v1(
        self,
        challenge: [u8; 32],
        sink: &mut OpaqueFrameSinkV1,
    ) -> Result<(), GlobalLookupErrorV1>;
}

struct OpaqueFrameSinkV1 {
    next_ordinal: usize,
    required: usize,
    state: [u8; 32],
}

impl OpaqueFrameSinkV1 {
    fn absorb_v1(&mut self, ordinal: usize, digest: [u8; 32]) -> Result<(), GlobalLookupErrorV1> {
        if ordinal != self.next_ordinal || ordinal >= self.required || digest == [0; 32] {
            return Err(GlobalLookupErrorV1::Order);
        }
        self.state = chain_frame_v1(self.state, b"opaque-stream-item", &digest);
        self.next_ordinal += 1;
        Ok(())
    }
}

enum SourcePackingOwnerSealV1 {
    Production {
        authenticated_source: Infallible,
        authenticated_packing: Infallible,
        same_opening: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}

enum LookupOwnerSealV1 {
    Production {
        canonical_planes: Infallible,
        z_dependent_inverses: Infallible,
        zero_sum_mask: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}

enum ProofOwnerSealV1 {
    Production {
        sumcheck: Infallible,
        endpoint_openings: Infallible,
        qpcs_same_opening: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}

struct BoundOwnerSealsV1 {
    source_packing_seal: SourcePackingOwnerSealV1,
    lookup_seal: LookupOwnerSealV1,
    proof_seal: ProofOwnerSealV1,
}

#[derive(Clone, Copy)]
struct BoundTranscriptFramesV1 {
    commitment_digest: [u8; 32],
    inverse_digest: [u8; 32],
    opening_digest: [u8; 32],
    existing_commitments: usize,
    added_commitments: usize,
    existing_inverses: usize,
    added_inverses: usize,
    cubic_messages: usize,
    hidden_endpoints: usize,
    multiplicity_commitments: usize,
    sumcheck_mask_commitments: usize,
    ipas: usize,
    table_ipas: usize,
    mask_ipas: usize,
    gates: usize,
}

impl BoundTranscriptFramesV1 {
    fn validate_v1(self) -> Result<(), GlobalLookupErrorV1> {
        let digests = [
            self.commitment_digest,
            self.inverse_digest,
            self.opening_digest,
        ];
        if digests.contains(&[0; 32])
            || (self.existing_commitments, self.added_commitments)
                != (EXISTING_ACTIVE_PLANES_V1, ADDED_PRE_Z_PLANES_V1)
            || (self.existing_inverses, self.added_inverses)
                != (EXISTING_ACTIVE_PLANES_V1, ADDED_INVERSE_PLANES_V1)
            || (
                self.cubic_messages,
                self.hidden_endpoints,
                self.multiplicity_commitments,
                self.sumcheck_mask_commitments,
                self.ipas,
                self.table_ipas,
                self.mask_ipas,
                self.gates,
            ) != (
                REQUIRED_CUBIC_MESSAGES_V1,
                HIDDEN_ENDPOINTS_V1,
                MULTIPLICITY_COMMITMENTS_V1,
                SUMCHECK_MASK_COMMITMENTS_V1,
                COEFFICIENT_IPAS_V1,
                TABLE_IPAS_V1,
                MASK_IPAS_V1,
                ENDPOINT_GATES_V1,
            )
        {
            return Err(GlobalLookupErrorV1::Shape);
        }
        Ok(())
    }
}

fn conditional_accounting_v1() -> Option<(usize, usize)> {
    CONDITIONAL_TOTAL_BYTES_V1.zip(CONDITIONAL_MARGIN_BYTES_V1)
}

#[cfg(test)]
#[path = "global_lookup_statement_v1_tests.rs"]
mod tests;
