//! Canonical source and public-statement preflight for the replacement RLWE link.
//!
//! This private module consumes the complete qPCS/FRI sequencing token, checks
//! the exact 43-record confidential-source encoding, binds the concrete
//! 40-limb public-artifact inventory, and freezes the two-equation/five-
//! repetition aggregation schedule.  It deliberately proves no equality
//! between those values and the qPCS rows.  Its move-only output is therefore
//! construction state only: it is not a proof receipt, readiness evidence, or
//! release authority, and the composite verifier remains fail-closed.

use super::{
    manifest::ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_CORRELATED_FRI_MAX_BYTES_V1,
        ZK_AMS_MKHE_RNS_NATIVE_INITIAL_MULTIPROOF_MAX_BYTES_V1, ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1,
        ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1, ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1,
        ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1, ZK_AMS_MKHE_RNS_NATIVE_RLWE_EQUATION_COUNT_V1,
        ZkAmsMkheRnsNativeFamilyV1, zk_ams_mkhe_rns_native_profile_manifest_v1,
        zk_ams_mkhe_rns_native_release_candidate_digest_v1, zk_ams_mkhe_rns_native_topology_v1,
    },
    rns_native_qpcs_fri_complete::{
        RnsNativeQpcsFriCompleteErrorV1, RnsNativeQpcsFriCompleteStageV1,
    },
    rns_native_qpcs_prefix::RnsNativeQpcsRelationScheduleV1,
    rns_native_section_codec::RNS_QPCS_FIXED_BYTES_V1,
    rns_native_source::{
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_BLOCKS_PER_OPENING_V1,
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1,
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_SLOTS_V1,
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_PLAINTEXT_BYTES_V1,
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_SLOTS_V1, ZkAmsMkheRnsNativeSecretChunkV1,
        ZkAmsMkheRnsNativeSourceArenaV1, ZkAmsMkheRnsNativeSourceLayoutV1,
        ZkAmsMkheRnsNativeSourceReceiptV1, ZkAmsMkheRnsNativeSourceSnapshotV1,
    },
    rns_native_transcript::ZkAmsMkheRnsNativeChallengeSeedsV1,
};
use crate::vega::{VEGA_T256_SCALAR_MODULUS_BE_V1, sponge::Keccak256};

const STATEMENT_VERSION_V1: u8 = 1;
const DIGEST_BYTES_V1: usize = 32;
const OPENING_COUNT_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1 as usize;
const EQUATION_COUNT_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_RLWE_EQUATION_COUNT_V1 as usize;
const REPETITION_COUNT_V1: usize = 5;
const ROWS_PER_REPETITION_V1: usize = 2;
const ROWS_PER_LIMB_V1: usize = REPETITION_COUNT_V1 * ROWS_PER_REPETITION_V1;
const COMPONENT_COUNT_V1: usize = 4;
const CANONICAL_COEFFICIENT_BYTES_V1: usize = 32;
const SIGNED_COEFFICIENT_BYTES_V1: usize = 8;
const CANONICAL_COEFFICIENTS_PER_BLOCK_V1: usize =
    ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1 as usize / CANONICAL_COEFFICIENT_BYTES_V1;
const SIGNED_COEFFICIENTS_PER_BLOCK_V1: usize =
    ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1 as usize / SIGNED_COEFFICIENT_BYTES_V1;
const CANONICAL_BLOCKS_PER_RECORD_V1: usize =
    ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 / CANONICAL_COEFFICIENTS_PER_BLOCK_V1;
const SIGNED_BLOCKS_PER_POLYNOMIAL_V1: usize =
    ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 / SIGNED_COEFFICIENTS_PER_BLOCK_V1;
const MAIN_BLOCKS_PER_RECORD_V1: usize =
    CANONICAL_BLOCKS_PER_RECORD_V1 + 3 * SIGNED_BLOCKS_PER_POLYNOMIAL_V1;
const PUBLIC_LIMB_DIGEST_COUNT_V1: usize = OPENING_COUNT_V1 * ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1;
pub(super) const RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ARTIFACT_DIGESTS_V1: usize =
    2 * ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 + 2 * PUBLIC_LIMB_DIGEST_COUNT_V1;
pub(super) const RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ARTIFACT_DIGEST_BYTES_V1: usize =
    RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ARTIFACT_DIGESTS_V1 * DIGEST_BYTES_V1;
pub(super) const RNS_NATIVE_PRETRANSCRIPT_SOURCE_READS_V1: u64 =
    ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_SLOTS_V1 + ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_SLOTS_V1;
pub(super) const RNS_NATIVE_PRETRANSCRIPT_SOURCE_PLAINTEXT_BYTES_V1: u64 =
    ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_SLOTS_V1
        * ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1
        + ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_SLOTS_V1
            * ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_PLAINTEXT_BYTES_V1;
pub(super) const RNS_NATIVE_PRETRANSCRIPT_CANONICAL_CHECKS_V1: u64 = OPENING_COUNT_V1 as u64
    * CANONICAL_BLOCKS_PER_RECORD_V1 as u64
    * CANONICAL_COEFFICIENTS_PER_BLOCK_V1 as u64;
pub(super) const RNS_NATIVE_PRETRANSCRIPT_SIGNED_CHECKS_V1: u64 = OPENING_COUNT_V1 as u64
    * 3
    * SIGNED_BLOCKS_PER_POLYNOMIAL_V1 as u64
    * SIGNED_COEFFICIENTS_PER_BLOCK_V1 as u64;
pub(super) const RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ALIAS_DIGESTS_V1: usize =
    3 + RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ARTIFACT_DIGESTS_V1 + 2 * OPENING_COUNT_V1;
pub(super) const RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ALIAS_BYTES_V1: usize =
    RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ALIAS_DIGESTS_V1 * DIGEST_BYTES_V1;
pub(super) const RNS_NATIVE_PRETRANSCRIPT_GLOBAL_ALIAS_DIGESTS_V1: usize = 20
    + 2 * OPENING_COUNT_V1
    + EQUATION_COUNT_V1
    + ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
    + 2 * ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
    + 2 * PUBLIC_LIMB_DIGEST_COUNT_V1
    + 2 * OPENING_COUNT_V1;
pub(super) const RNS_NATIVE_PRETRANSCRIPT_GLOBAL_ALIAS_BYTES_V1: usize =
    RNS_NATIVE_PRETRANSCRIPT_GLOBAL_ALIAS_DIGESTS_V1 * DIGEST_BYTES_V1;
const RNS_NATIVE_PRETRANSCRIPT_PUBLIC_KEY_HASH_BYTES_V1: u64 = PUBLIC_KEY_DOMAIN_V1.len() as u64
    + 1
    + 6 * DIGEST_BYTES_V1 as u64
    + 8
    + DIGEST_BYTES_V1 as u64
    + ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u64 * (1 + 8 + 2 * DIGEST_BYTES_V1 as u64);
const RNS_NATIVE_PRETRANSCRIPT_NONCE_HASH_BYTES_V1: u64 = NONCE_BINDING_DOMAIN_V1.len() as u64
    + 1
    + 6 * DIGEST_BYTES_V1 as u64
    + 8
    + 2 * DIGEST_BYTES_V1 as u64
    + 4
    + 4
    + 8
    + ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_PLAINTEXT_BYTES_V1;
const RNS_NATIVE_PRETRANSCRIPT_RECORD_HASH_BYTES_V1: u64 = PUBLIC_RECORD_DOMAIN_V1.len() as u64
    + 1
    + DIGEST_BYTES_V1 as u64
    + 4
    + 4
    + 8
    + DIGEST_BYTES_V1 as u64
    + ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u64 * (1 + 8 + 2 * DIGEST_BYTES_V1 as u64);
const RNS_NATIVE_PRETRANSCRIPT_BUNDLE_HASH_BYTES_V1: u64 = PUBLIC_BUNDLE_DOMAIN_V1.len() as u64
    + 1
    + 6 * DIGEST_BYTES_V1 as u64
    + 8
    + 2 * DIGEST_BYTES_V1 as u64
    + OPENING_COUNT_V1 as u64 * (3 + 8 + 2 * DIGEST_BYTES_V1 as u64);
/// Exact bytes absorbed by the existing public-key, 43 nonce-binding, 43
/// record, and public-bundle digest languages during the pre-transcript pass.
pub(super) const RNS_NATIVE_PRETRANSCRIPT_PUBLIC_DIGEST_HASH_BYTES_V1: u64 =
    RNS_NATIVE_PRETRANSCRIPT_PUBLIC_KEY_HASH_BYTES_V1
        + OPENING_COUNT_V1 as u64
            * (RNS_NATIVE_PRETRANSCRIPT_NONCE_HASH_BYTES_V1
                + RNS_NATIVE_PRETRANSCRIPT_RECORD_HASH_BYTES_V1)
        + RNS_NATIVE_PRETRANSCRIPT_BUNDLE_HASH_BYTES_V1;
const QPCS_EVALUATION_BYTES_V1: usize =
    ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * REPETITION_COUNT_V1 * ROWS_PER_REPETITION_V1 * 8;
const MAX_CHALLENGE_ATTEMPTS_V1: u16 = 256;

const QPCS_NESTED_HEADER_BYTES_V1: usize = 186 + 476 + 416;
/// Absolute bytes available to the residual after worst-case qPCS and section framing.
pub(super) const RNS_NATIVE_RLWE_SOURCE_RESIDUAL_MAX_BYTES_V1: usize =
    ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1 as usize
        - RNS_QPCS_FIXED_BYTES_V1
        - ZK_AMS_MKHE_RNS_NATIVE_INITIAL_MULTIPROOF_MAX_BYTES_V1 as usize
        - ZK_AMS_MKHE_RNS_NATIVE_CORRELATED_FRI_MAX_BYTES_V1 as usize
        - QPCS_NESTED_HEADER_BYTES_V1
        - QPCS_EVALUATION_BYTES_V1;
const ANCHOR_MAGIC_V1: [u8; 4] = *b"ZRLS";
const ANCHOR_CORE_DIGESTS_V1: usize = 23;
const CORE_QPCS_PARAMETER_V1: usize = 0;
const CORE_TRANSCRIPT_V1: usize = 1;
const CORE_QUERY_SEED_V1: usize = 2;
const CORE_QPCS_SECTION_V1: usize = 3;
const CORE_FRI_SCHEDULE_V1: usize = 4;
const CORE_PROFILE_V1: usize = 5;
const CORE_TOPOLOGY_V1: usize = 6;
const CORE_RELEASE_CANDIDATE_V1: usize = 7;
const CORE_SOURCE_BINDING_V1: usize = 8;
const CORE_MAIN_SNAPSHOT_V1: usize = 9;
const CORE_NONCE_SNAPSHOT_V1: usize = 10;
const CORE_SOURCE_RECEIPT_V1: usize = 11;
const CORE_STATEMENT_V1: usize = 12;
const CORE_OPERATIONAL_V1: usize = 13;
const CORE_ROSTER_V1: usize = 14;
const CORE_PUBLIC_BUNDLE_V1: usize = 15;
const CORE_OPENING_BUNDLE_V1: usize = 16;
const CORE_FORMULA_V1: usize = 17;
const CORE_MAPPING_V1: usize = 18;
const CORE_EQUATION_BUNDLE_V1: usize = 19;
const CORE_LIMB_BUNDLE_V1: usize = 20;
const CORE_AGGREGATION_SCHEDULE_V1: usize = 21;
const CORE_DOWNSTREAM_V1: usize = 22;
const ANCHOR_HEADER_BYTES_V1: usize = 4 + 1 + 1 + 5 + 1 + 4 + 3 * 2 + 8 + 4;
const ANCHOR_FIXED_BYTES_V1: usize =
    ANCHOR_HEADER_BYTES_V1 + ANCHOR_CORE_DIGESTS_V1 * DIGEST_BYTES_V1;
pub(super) const RNS_NATIVE_RLWE_SOURCE_DOWNSTREAM_MAX_BYTES_V1: usize =
    RNS_NATIVE_RLWE_SOURCE_RESIDUAL_MAX_BYTES_V1 - ANCHOR_FIXED_BYTES_V1;

const SOURCE_SEMANTICS_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-rlwe-source.semantics";
const RECORD_MAPPING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-rlwe-source.record-mapping";
const RLWE_FORMULA_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-rlwe-source.formula";
const PUBLIC_KEY_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-rlwe-source.public-key";
const PUBLIC_RECORD_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-rlwe-source.public-record";
const PUBLIC_BUNDLE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-rlwe-source.public-bundle";
const PUBLIC_STATEMENT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-rlwe-source.public-statement";
const NONCE_BINDING_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-rlwe-source.nonce-binding";
const AGGREGATION_CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-rlwe-source.aggregation-challenge";
const AGGREGATION_SCHEDULE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-rlwe-source.aggregation-schedule";
const ANCHOR_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-rlwe-source.residual-anchor";
const DOWNSTREAM_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-rlwe-source.downstream";
const OPENING_BUNDLE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-rlwe-source.opening-bundle";
const EQUATION_BUNDLE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-rlwe-source.equation-bundle";
const LIMB_BUNDLE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-rlwe-source.limb-bundle";
const EVALUATION_BYTES_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-rlwe-source.qpcs-evaluations";

const CANONICAL_SOURCE_SEMANTICS_V1: &[u8] =
    b"M:256x32-byte-big-endian;strictly-less-than-T256-scalar-modulus";
const EPHEMERAL_SOURCE_SEMANTICS_V1: &[u8] =
    b"r:1024xi64-big-endian;coefficients-in{-1,0,1};polynomial-nonzero";
const ERROR_SOURCE_SEMANTICS_V1: &[u8] = b"e0,e1:1024xi64-big-endian;absolute-value-at-most-2";
const NONCE_SOURCE_SEMANTICS_V1: &[u8] = b"nonce:32-byte;nonzero;sample-epoch-roster-bound";
const USED_SLOT_SEMANTICS_V1: &[u8] =
    b"used-slots-apply-after-packing-and-global-lookup;raw-coefficient-tail-not-zero-checked";

const ORDINARY_PRODUCT_FORMULA_V1: &[u8] =
    b"T[j,e,l]=ordinary(K[e,l]*r[j,l]);degree(T)<=2N-2;len(T)=2N";
const PLAIN_MODULUS_LIMB_FORMULA_V1: &[u8] = b"p_l=T256_scalar_modulus mod q_l";
const QUOTIENT_FORMULA_V1: &[u8] = b"H[j,e,l][i]=T[j,e,l][N+i];H[N-1]=0";
const RELATION_FORMULA_V1: &[u8] = b"P=T+p_l*E+delta*M-C=(X^N+1)*H mod q_l";
const TOP_ZERO_FORMULA_V1: &[u8] = b"P[2N-1]=H[N-1]=0";
const CENTERING_FORMULA_V1: &[u8] = b"M_l=m_if_m<=(p-1)/2_else_m-p;then_canonical_mod_q_l";
const EQUATION_ZERO_FORMULA_V1: &[u8] = b"e=0:K=B,E=e0,C=C0,delta=1";
const EQUATION_ONE_FORMULA_V1: &[u8] = b"e=1:K=A,E=e1,C=C1,delta=0";
const AGGREGATE_FORMULA_V1: &[u8] = b"sum_j gamma_lk^j*(equation0+beta_lk*equation1)";
const AGGREGATE_EQUIVALENT_FORMULA_V1: &[u8] = b"R=sum_j gamma^j*r_j;K=B+beta*A;E=sum_j gamma^j*(e0_j+beta*e1_j);M=sum_j gamma^j*M_j;C=sum_j gamma^j*(C0_j+beta*C1_j);P=ordinary(K*R)+p_l*E+M-C";
const MASKING_FORMULA_V1: &[u8] =
    b"private-uniform-S:deg<=N-2;H~=H+S;P~=P+(X^N+1)S;commit-before-z";
const EMISSION_ORDER_V1: &[u8] = b"limb->repetition->row(Product,OpeningQuotient);40*5*2=400";

const _: () = {
    assert!(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 == 131_072);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 == 40);
    assert!(OPENING_COUNT_V1 == 43);
    assert!(EQUATION_COUNT_V1 == 2);
    assert!(CANONICAL_COEFFICIENTS_PER_BLOCK_V1 == 256);
    assert!(SIGNED_COEFFICIENTS_PER_BLOCK_V1 == 1_024);
    assert!(CANONICAL_BLOCKS_PER_RECORD_V1 == 512);
    assert!(SIGNED_BLOCKS_PER_POLYNOMIAL_V1 == 128);
    assert!(MAIN_BLOCKS_PER_RECORD_V1 == 896);
    assert!(
        MAIN_BLOCKS_PER_RECORD_V1 as u64
            == ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_BLOCKS_PER_OPENING_V1
    );
    assert!(
        OPENING_COUNT_V1 * MAIN_BLOCKS_PER_RECORD_V1
            == ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_SLOTS_V1 as usize
    );
    assert!(OPENING_COUNT_V1 == ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_SLOTS_V1 as usize);
    assert!(QPCS_EVALUATION_BYTES_V1 == 3_200);
    assert!(ROWS_PER_LIMB_V1 == 10);
    assert!(PUBLIC_LIMB_DIGEST_COUNT_V1 == 1_720);
    assert!(RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ARTIFACT_DIGESTS_V1 == 3_520);
    assert!(RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ARTIFACT_DIGEST_BYTES_V1 == 112_640);
    assert!(RNS_NATIVE_PRETRANSCRIPT_SOURCE_READS_V1 == 38_571);
    assert!(RNS_NATIVE_PRETRANSCRIPT_SOURCE_PLAINTEXT_BYTES_V1 == 315_622_752);
    assert!(RNS_NATIVE_PRETRANSCRIPT_CANONICAL_CHECKS_V1 == 5_636_096);
    assert!(RNS_NATIVE_PRETRANSCRIPT_SIGNED_CHECKS_V1 == 16_908_288);
    assert!(RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ALIAS_DIGESTS_V1 == 3_609);
    assert!(RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ALIAS_BYTES_V1 == 115_488);
    assert!(RNS_NATIVE_PRETRANSCRIPT_GLOBAL_ALIAS_DIGESTS_V1 == 3_754);
    assert!(RNS_NATIVE_PRETRANSCRIPT_GLOBAL_ALIAS_BYTES_V1 == 120_128);
    assert!(RNS_NATIVE_PRETRANSCRIPT_PUBLIC_KEY_HASH_BYTES_V1 == 3_207);
    assert!(RNS_NATIVE_PRETRANSCRIPT_NONCE_HASH_BYTES_V1 == 370);
    assert!(RNS_NATIVE_PRETRANSCRIPT_RECORD_HASH_BYTES_V1 == 3_058);
    assert!(RNS_NATIVE_PRETRANSCRIPT_BUNDLE_HASH_BYTES_V1 == 3_547);
    assert!(RNS_NATIVE_PRETRANSCRIPT_PUBLIC_DIGEST_HASH_BYTES_V1 == 154_158);
    assert!(ANCHOR_HEADER_BYTES_V1 == 34);
    assert!(ANCHOR_FIXED_BYTES_V1 == 770);
    assert!(RNS_NATIVE_RLWE_SOURCE_DOWNSTREAM_MAX_BYTES_V1 == 3_783);
    assert!(RNS_QPCS_FIXED_BYTES_V1 == 8_449);
    assert!(QPCS_NESTED_HEADER_BYTES_V1 == 1_078);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeRlweSourceStatementErrorV1 {
    InvalidContext,
    InvalidPublicArtifact,
    InvalidAnchor,
    AnchorCapExceeded,
    InvalidSourceOrder,
    InvalidSourceEncoding,
    NonCanonicalPlaintext,
    InvalidEphemeral,
    InvalidError,
    InvalidNonce,
    InvalidChallenge,
    SourceUnavailable,
    ArithmeticOverflow,
}

impl core::fmt::Display for RnsNativeRlweSourceStatementErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeRlweSourceStatementErrorV1 {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum SourceComponentV1 {
    CanonicalPlaintext = 1,
    Ephemeral = 2,
    ErrorZero = 3,
    ErrorOne = 4,
}

impl SourceComponentV1 {
    const fn first_block(self) -> usize {
        match self {
            Self::CanonicalPlaintext => 0,
            Self::Ephemeral => CANONICAL_BLOCKS_PER_RECORD_V1,
            Self::ErrorZero => CANONICAL_BLOCKS_PER_RECORD_V1 + SIGNED_BLOCKS_PER_POLYNOMIAL_V1,
            Self::ErrorOne => CANONICAL_BLOCKS_PER_RECORD_V1 + 2 * SIGNED_BLOCKS_PER_POLYNOMIAL_V1,
        }
    }

    const fn block_count(self) -> usize {
        match self {
            Self::CanonicalPlaintext => CANONICAL_BLOCKS_PER_RECORD_V1,
            Self::Ephemeral | Self::ErrorZero | Self::ErrorOne => SIGNED_BLOCKS_PER_POLYNOMIAL_V1,
        }
    }
}

const SOURCE_COMPONENT_ORDER_V1: [SourceComponentV1; COMPONENT_COUNT_V1] = [
    SourceComponentV1::CanonicalPlaintext,
    SourceComponentV1::Ephemeral,
    SourceComponentV1::ErrorZero,
    SourceComponentV1::ErrorOne,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RecordPositionV1 {
    ordinal: u8,
    family: ZkAmsMkheRnsNativeFamilyV1,
    family_index: u8,
    used_slots: u32,
}

fn record_position_v1(ordinal: usize) -> Option<RecordPositionV1> {
    let ordinal_u8 = u8::try_from(ordinal).ok()?;
    let (family, family_index, used_slots) = match ordinal {
        0 => (ZkAmsMkheRnsNativeFamilyV1::X, 0, 89),
        1..=16 => (
            ZkAmsMkheRnsNativeFamilyV1::U,
            u8::try_from(ordinal - 1).ok()?,
            65_536,
        ),
        17..=32 => (
            ZkAmsMkheRnsNativeFamilyV1::E,
            u8::try_from(ordinal - 17).ok()?,
            65_536,
        ),
        33 => (ZkAmsMkheRnsNativeFamilyV1::RE, 0, 1_024),
        34..=41 => (
            ZkAmsMkheRnsNativeFamilyV1::W,
            u8::try_from(ordinal - 34).ok()?,
            65_536,
        ),
        42 => (ZkAmsMkheRnsNativeFamilyV1::RW, 0, 512),
        _ => return None,
    };
    Some(RecordPositionV1 {
        ordinal: ordinal_u8,
        family,
        family_index,
        used_slots,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativePublicRecordMetadataV1 {
    ordinal: u8,
    family: ZkAmsMkheRnsNativeFamilyV1,
    family_index: u8,
    sample_index: u64,
    nonce_binding_digest: [u8; DIGEST_BYTES_V1],
    record_digest: [u8; DIGEST_BYTES_V1],
}

impl RnsNativePublicRecordMetadataV1 {
    pub(super) const fn new(
        ordinal: u8,
        family: ZkAmsMkheRnsNativeFamilyV1,
        family_index: u8,
        sample_index: u64,
        nonce_binding_digest: [u8; DIGEST_BYTES_V1],
        record_digest: [u8; DIGEST_BYTES_V1],
    ) -> Self {
        Self {
            ordinal,
            family,
            family_index,
            sample_index,
            nonce_binding_digest,
            record_digest,
        }
    }
}

/// Concrete borrowed inventory for the exact 40-limb public key and 43 ciphertexts.
///
/// Limb digests are canonical artifact identities, not proof commitments.  This
/// view only freezes the public statement consumed by the later proof builder.
#[derive(Clone, Copy)]
pub(super) struct RnsNativePublicArtifactViewV1<'a> {
    epoch: u64,
    governed_roster_digest: [u8; DIGEST_BYTES_V1],
    public_a_limb_digests: &'a [[u8; DIGEST_BYTES_V1]],
    public_b_limb_digests: &'a [[u8; DIGEST_BYTES_V1]],
    ciphertext_c0_limb_digests: &'a [[u8; DIGEST_BYTES_V1]],
    ciphertext_c1_limb_digests: &'a [[u8; DIGEST_BYTES_V1]],
    records: &'a [RnsNativePublicRecordMetadataV1],
    public_bundle_digest: [u8; DIGEST_BYTES_V1],
}

impl<'a> RnsNativePublicArtifactViewV1<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) const fn new(
        epoch: u64,
        governed_roster_digest: [u8; DIGEST_BYTES_V1],
        public_a_limb_digests: &'a [[u8; DIGEST_BYTES_V1]],
        public_b_limb_digests: &'a [[u8; DIGEST_BYTES_V1]],
        ciphertext_c0_limb_digests: &'a [[u8; DIGEST_BYTES_V1]],
        ciphertext_c1_limb_digests: &'a [[u8; DIGEST_BYTES_V1]],
        records: &'a [RnsNativePublicRecordMetadataV1],
        public_bundle_digest: [u8; DIGEST_BYTES_V1],
    ) -> Self {
        Self {
            epoch,
            governed_roster_digest,
            public_a_limb_digests,
            public_b_limb_digests,
            ciphertext_c0_limb_digests,
            ciphertext_c1_limb_digests,
            records,
            public_bundle_digest,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum RlweEquationV1 {
    Constant = 0,
    Linear = 1,
}

impl RlweEquationV1 {
    const fn key_role(self) -> u8 {
        match self {
            Self::Constant => b'B',
            Self::Linear => b'A',
        }
    }

    const fn error_role(self) -> SourceComponentV1 {
        match self {
            Self::Constant => SourceComponentV1::ErrorZero,
            Self::Linear => SourceComponentV1::ErrorOne,
        }
    }

    const fn ciphertext_role(self) -> u8 {
        match self {
            Self::Constant => 0,
            Self::Linear => 1,
        }
    }

    const fn plaintext_delta(self) -> u8 {
        match self {
            Self::Constant => 1,
            Self::Linear => 0,
        }
    }
}

const RLWE_EQUATIONS_V1: [RlweEquationV1; EQUATION_COUNT_V1] =
    [RlweEquationV1::Constant, RlweEquationV1::Linear];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum QpcsRowRoleV1 {
    Product = 0,
    OpeningQuotient = 1,
}

const QPCS_ROW_ROLES_V1: [QpcsRowRoleV1; ROWS_PER_REPETITION_V1] =
    [QpcsRowRoleV1::Product, QpcsRowRoleV1::OpeningQuotient];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AggregationChallengeV1 {
    gamma: u64,
    beta: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ResidualAnchorV1<'a> {
    epoch: u64,
    core_digests: [[u8; DIGEST_BYTES_V1]; ANCHOR_CORE_DIGESTS_V1],
    downstream: &'a [u8],
}

struct DecoderV1<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> DecoderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], RnsNativeRlweSourceStatementErrorV1> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?;
        let output = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)?;
        self.cursor = end;
        Ok(output)
    }

    fn u8(&mut self) -> Result<u8, RnsNativeRlweSourceStatementErrorV1> {
        Ok(*self
            .take(1)?
            .first()
            .ok_or(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)?)
    }

    fn u16(&mut self) -> Result<u16, RnsNativeRlweSourceStatementErrorV1> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().map_err(
            |_| RnsNativeRlweSourceStatementErrorV1::InvalidAnchor,
        )?))
    }

    fn u32(&mut self) -> Result<u32, RnsNativeRlweSourceStatementErrorV1> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().map_err(
            |_| RnsNativeRlweSourceStatementErrorV1::InvalidAnchor,
        )?))
    }

    fn u64(&mut self) -> Result<u64, RnsNativeRlweSourceStatementErrorV1> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().map_err(
            |_| RnsNativeRlweSourceStatementErrorV1::InvalidAnchor,
        )?))
    }

    fn digest(&mut self) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRlweSourceStatementErrorV1> {
        self.take(DIGEST_BYTES_V1)?
            .try_into()
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)
    }
}

struct DigestRegistryV1 {
    digests: Vec<[u8; DIGEST_BYTES_V1]>,
}

impl DigestRegistryV1 {
    fn with_capacity_v1(capacity: usize) -> Result<Self, RnsNativeRlweSourceStatementErrorV1> {
        let mut digests = Vec::new();
        digests
            .try_reserve_exact(capacity)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?;
        Ok(Self { digests })
    }

    fn insert_v1(
        &mut self,
        digest: [u8; DIGEST_BYTES_V1],
    ) -> Result<(), RnsNativeRlweSourceStatementErrorV1> {
        if digest == [0; DIGEST_BYTES_V1] {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact);
        }
        let Err(insertion) = self.digests.binary_search(&digest) else {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact);
        };
        self.digests.insert(insertion, digest);
        Ok(())
    }
}

impl<'a> ResidualAnchorV1<'a> {
    fn from_parts_v1(
        epoch: u64,
        core_digests: [[u8; DIGEST_BYTES_V1]; ANCHOR_CORE_DIGESTS_V1],
        downstream: &'a [u8],
    ) -> Result<Self, RnsNativeRlweSourceStatementErrorV1> {
        let anchor = Self {
            epoch,
            core_digests,
            downstream,
        };
        anchor.validate_v1()?;
        Ok(anchor)
    }

    fn from_canonical_bytes_exact_v1(
        bytes: &'a [u8],
    ) -> Result<Self, RnsNativeRlweSourceStatementErrorV1> {
        if bytes.len() > RNS_NATIVE_RLWE_SOURCE_RESIDUAL_MAX_BYTES_V1 {
            return Err(RnsNativeRlweSourceStatementErrorV1::AnchorCapExceeded);
        }
        if bytes.len() <= ANCHOR_FIXED_BYTES_V1 {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor);
        }
        let mut decoder = DecoderV1::new(bytes);
        if decoder.take(ANCHOR_MAGIC_V1.len())? != ANCHOR_MAGIC_V1.as_slice()
            || decoder.u8()? != STATEMENT_VERSION_V1
            || decoder.u8()? != 0
            || usize::from(decoder.u8()?) != OPENING_COUNT_V1
            || usize::from(decoder.u8()?) != EQUATION_COUNT_V1
            || usize::from(decoder.u8()?) != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
            || usize::from(decoder.u8()?) != REPETITION_COUNT_V1
            || usize::from(decoder.u8()?) != COMPONENT_COUNT_V1
            || decoder.u8()? != 0
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)?
                != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
            || usize::from(decoder.u16()?) != MAIN_BLOCKS_PER_RECORD_V1
            || usize::from(decoder.u16()?) != CANONICAL_BLOCKS_PER_RECORD_V1
            || usize::from(decoder.u16()?) != SIGNED_BLOCKS_PER_POLYNOMIAL_V1
        {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor);
        }
        let epoch = decoder.u64()?;
        let downstream_len = usize::try_from(decoder.u32()?)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)?;
        let expected_len = ANCHOR_FIXED_BYTES_V1
            .checked_add(downstream_len)
            .ok_or(RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?;
        if downstream_len == 0
            || downstream_len > RNS_NATIVE_RLWE_SOURCE_DOWNSTREAM_MAX_BYTES_V1
            || expected_len != bytes.len()
        {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor);
        }
        let mut core_digests = [[0; DIGEST_BYTES_V1]; ANCHOR_CORE_DIGESTS_V1];
        for digest in &mut core_digests {
            *digest = decoder.digest()?;
        }
        let downstream = decoder.take(downstream_len)?;
        if decoder.cursor != bytes.len() {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor);
        }
        let anchor = Self::from_parts_v1(epoch, core_digests, downstream)?;
        if anchor.to_canonical_bytes_v1()?.as_slice() != bytes {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor);
        }
        Ok(anchor)
    }

    fn to_canonical_bytes_v1(self) -> Result<Vec<u8>, RnsNativeRlweSourceStatementErrorV1> {
        self.validate_v1()?;
        let encoded_len = ANCHOR_FIXED_BYTES_V1
            .checked_add(self.downstream.len())
            .ok_or(RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?;
        if encoded_len > RNS_NATIVE_RLWE_SOURCE_RESIDUAL_MAX_BYTES_V1 {
            return Err(RnsNativeRlweSourceStatementErrorV1::AnchorCapExceeded);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(encoded_len)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?;
        bytes.extend_from_slice(&ANCHOR_MAGIC_V1);
        bytes.extend_from_slice(&[
            STATEMENT_VERSION_V1,
            0,
            u8::try_from(OPENING_COUNT_V1)
                .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
            u8::try_from(EQUATION_COUNT_V1)
                .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
            u8::try_from(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1)
                .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
            u8::try_from(REPETITION_COUNT_V1)
                .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
            u8::try_from(COMPONENT_COUNT_V1)
                .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
            0,
        ]);
        bytes.extend_from_slice(
            &u32::try_from(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)
                .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?
                .to_be_bytes(),
        );
        for count in [
            MAIN_BLOCKS_PER_RECORD_V1,
            CANONICAL_BLOCKS_PER_RECORD_V1,
            SIGNED_BLOCKS_PER_POLYNOMIAL_V1,
        ] {
            bytes.extend_from_slice(
                &u16::try_from(count)
                    .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?
                    .to_be_bytes(),
            );
        }
        bytes.extend_from_slice(&self.epoch.to_be_bytes());
        bytes.extend_from_slice(
            &u32::try_from(self.downstream.len())
                .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?
                .to_be_bytes(),
        );
        for digest in self.core_digests {
            bytes.extend_from_slice(&digest);
        }
        bytes.extend_from_slice(self.downstream);
        if bytes.len() != encoded_len {
            return Err(RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow);
        }
        Ok(bytes)
    }

    fn validate_v1(self) -> Result<(), RnsNativeRlweSourceStatementErrorV1> {
        if self.epoch == 0
            || self.downstream.is_empty()
            || self.downstream.len() > RNS_NATIVE_RLWE_SOURCE_DOWNSTREAM_MAX_BYTES_V1
            || self.core_digests[ANCHOR_CORE_DIGESTS_V1 - 1]
                != downstream_digest_v1(self.downstream)?
        {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor);
        }
        let mut registry = DigestRegistryV1::with_capacity_v1(ANCHOR_CORE_DIGESTS_V1)?;
        for digest in self.core_digests {
            registry
                .insert_v1(digest)
                .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)?;
        }
        Ok(())
    }
}

fn downstream_digest_v1(
    downstream: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRlweSourceStatementErrorV1> {
    if downstream.is_empty() || downstream.len() > RNS_NATIVE_RLWE_SOURCE_DOWNSTREAM_MAX_BYTES_V1 {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor);
    }
    let mut hash = Keccak256::new();
    hash.update(DOWNSTREAM_DIGEST_DOMAIN_V1);
    hash.update(&[STATEMENT_VERSION_V1]);
    hash.update(
        &u32::try_from(downstream.len())
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(downstream);
    nonzero_digest_v1(hash.finalize())
        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)
}

fn nonzero_digest_v1(
    digest: [u8; DIGEST_BYTES_V1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRlweSourceStatementErrorV1> {
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidContext);
    }
    Ok(digest)
}

fn source_semantics_digest_v1() -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRlweSourceStatementErrorV1>
{
    let mut hash = Keccak256::new();
    hash.update(SOURCE_SEMANTICS_DOMAIN_V1);
    hash.update(&[STATEMENT_VERSION_V1]);
    hash.update(
        &u32::try_from(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    for value in [
        MAIN_BLOCKS_PER_RECORD_V1,
        CANONICAL_BLOCKS_PER_RECORD_V1,
        SIGNED_BLOCKS_PER_POLYNOMIAL_V1,
        CANONICAL_COEFFICIENTS_PER_BLOCK_V1,
        SIGNED_COEFFICIENTS_PER_BLOCK_V1,
        CANONICAL_COEFFICIENT_BYTES_V1,
        SIGNED_COEFFICIENT_BYTES_V1,
    ] {
        hash.update(
            &u32::try_from(value)
                .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?
                .to_be_bytes(),
        );
    }
    for value in [-1_i64, 0, 1] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&2_i64.to_be_bytes());
    hash.update(
        &u16::try_from(ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_PLAINTEXT_BYTES_V1)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    for semantics in [
        CANONICAL_SOURCE_SEMANTICS_V1,
        EPHEMERAL_SOURCE_SEMANTICS_V1,
        ERROR_SOURCE_SEMANTICS_V1,
        NONCE_SOURCE_SEMANTICS_V1,
        USED_SLOT_SEMANTICS_V1,
    ] {
        hash.update(
            &u16::try_from(semantics.len())
                .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?
                .to_be_bytes(),
        );
        hash.update(semantics);
    }
    hash.update(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    nonzero_digest_v1(hash.finalize())
}

fn rlwe_formula_digest_v1() -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRlweSourceStatementErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(RLWE_FORMULA_DOMAIN_V1);
    hash.update(&[STATEMENT_VERSION_V1]);
    hash.update(&source_semantics_digest_v1()?);
    hash.update(
        &u32::try_from(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    for (limb, modulus) in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1.into_iter().enumerate() {
        hash.update(&[u8::try_from(limb)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?]);
        hash.update(&modulus.to_be_bytes());
    }
    for equation in RLWE_EQUATIONS_V1 {
        hash.update(&[
            equation as u8,
            equation.key_role(),
            equation.error_role() as u8,
            equation.ciphertext_role(),
            equation.plaintext_delta(),
        ]);
    }
    for formula in [
        ORDINARY_PRODUCT_FORMULA_V1,
        PLAIN_MODULUS_LIMB_FORMULA_V1,
        QUOTIENT_FORMULA_V1,
        RELATION_FORMULA_V1,
        TOP_ZERO_FORMULA_V1,
        CENTERING_FORMULA_V1,
        EQUATION_ZERO_FORMULA_V1,
        EQUATION_ONE_FORMULA_V1,
        AGGREGATE_FORMULA_V1,
        AGGREGATE_EQUIVALENT_FORMULA_V1,
        MASKING_FORMULA_V1,
        EMISSION_ORDER_V1,
    ] {
        hash.update(
            &u16::try_from(formula.len())
                .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?
                .to_be_bytes(),
        );
        hash.update(formula);
    }
    nonzero_digest_v1(hash.finalize())
}

fn record_mapping_digest_v1() -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRlweSourceStatementErrorV1>
{
    let mut hash = Keccak256::new();
    hash.update(RECORD_MAPPING_DOMAIN_V1);
    hash.update(&[STATEMENT_VERSION_V1]);
    hash.update(&source_semantics_digest_v1()?);
    hash.update(&[
        u8::try_from(OPENING_COUNT_V1)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
        u8::try_from(COMPONENT_COUNT_V1)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
        u8::try_from(EQUATION_COUNT_V1)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
        u8::try_from(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
        u8::try_from(REPETITION_COUNT_V1)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
        u8::try_from(ROWS_PER_REPETITION_V1)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
    ]);
    for ordinal in 0..OPENING_COUNT_V1 {
        let position = record_position_v1(ordinal)
            .ok_or(RnsNativeRlweSourceStatementErrorV1::InvalidSourceOrder)?;
        hash.update(&[
            position.ordinal,
            position.family as u8,
            position.family_index,
            position.family.record_count(),
        ]);
        hash.update(&position.used_slots.to_be_bytes());
        hash.update(
            &u64::try_from(ordinal * MAIN_BLOCKS_PER_RECORD_V1)
                .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?
                .to_be_bytes(),
        );
        hash.update(
            &u64::try_from(ordinal)
                .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?
                .to_be_bytes(),
        );
    }
    for component in SOURCE_COMPONENT_ORDER_V1 {
        let (coefficient_bytes, encoding_tag) = match component {
            SourceComponentV1::CanonicalPlaintext => (CANONICAL_COEFFICIENT_BYTES_V1, 1_u8),
            SourceComponentV1::Ephemeral
            | SourceComponentV1::ErrorZero
            | SourceComponentV1::ErrorOne => (SIGNED_COEFFICIENT_BYTES_V1, 2_u8),
        };
        hash.update(&[component as u8, encoding_tag]);
        for value in [
            component.first_block(),
            component.block_count(),
            coefficient_bytes,
        ] {
            hash.update(
                &u32::try_from(value)
                    .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?
                    .to_be_bytes(),
            );
        }
    }
    for record in 0..OPENING_COUNT_V1 {
        for equation in RLWE_EQUATIONS_V1 {
            for (limb, modulus) in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1.into_iter().enumerate() {
                hash.update(&[
                    u8::try_from(record)
                        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
                    equation as u8,
                    u8::try_from(limb)
                        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
                ]);
                hash.update(&modulus.to_be_bytes());
            }
        }
    }
    for limb in 0..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
        for repetition in 0..REPETITION_COUNT_V1 {
            for role in QPCS_ROW_ROLES_V1 {
                let row = repetition * ROWS_PER_REPETITION_V1 + role as usize;
                hash.update(&[
                    u8::try_from(limb)
                        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
                    u8::try_from(repetition)
                        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
                    role as u8,
                    u8::try_from(row)
                        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
                ]);
            }
        }
    }
    nonzero_digest_v1(hash.finalize())
}

fn opening_bundle_digest_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRlweSourceStatementErrorV1> {
    let mut registry = DigestRegistryV1::with_capacity_v1(OPENING_COUNT_V1 * 2)?;
    let mut hash = Keccak256::new();
    hash.update(OPENING_BUNDLE_DOMAIN_V1);
    hash.update(&[STATEMENT_VERSION_V1]);
    hash.update(
        &u16::try_from(OPENING_COUNT_V1)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    for (ordinal, opening) in transcript.opening_commitments().iter().enumerate() {
        let expected = record_position_v1(ordinal)
            .ok_or(RnsNativeRlweSourceStatementErrorV1::InvalidContext)?;
        if opening.family() != expected.family || opening.family_index() != expected.family_index {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidContext);
        }
        let source = opening.source_commitment_digest();
        let hyrax = opening.hyrax_commitment_digest();
        registry.insert_v1(source)?;
        registry.insert_v1(hyrax)?;
        hash.update(&[
            expected.ordinal,
            expected.family as u8,
            expected.family_index,
        ]);
        hash.update(&source);
        hash.update(&hyrax);
    }
    nonzero_digest_v1(hash.finalize())
}

fn equation_bundle_digest_v1(
    equation_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRlweSourceStatementErrorV1> {
    if equation_commitment_digests.len() != EQUATION_COUNT_V1 {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidContext);
    }
    let mut registry = DigestRegistryV1::with_capacity_v1(EQUATION_COUNT_V1)?;
    let mut hash = Keccak256::new();
    hash.update(EQUATION_BUNDLE_DOMAIN_V1);
    hash.update(&[STATEMENT_VERSION_V1]);
    for (ordinal, digest) in equation_commitment_digests.iter().enumerate() {
        registry.insert_v1(*digest)?;
        hash.update(&[u8::try_from(ordinal)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?]);
        hash.update(digest);
    }
    nonzero_digest_v1(hash.finalize())
}

fn limb_bundle_digest_v1(
    limb_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRlweSourceStatementErrorV1> {
    if limb_commitment_digests.len() != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidContext);
    }
    let mut registry = DigestRegistryV1::with_capacity_v1(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1)?;
    let mut hash = Keccak256::new();
    hash.update(LIMB_BUNDLE_DOMAIN_V1);
    hash.update(&[STATEMENT_VERSION_V1]);
    for (limb, (modulus, digest)) in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1
        .into_iter()
        .zip(limb_commitment_digests)
        .enumerate()
    {
        registry.insert_v1(*digest)?;
        hash.update(&[u8::try_from(limb)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?]);
        hash.update(&modulus.to_be_bytes());
        hash.update(digest);
    }
    nonzero_digest_v1(hash.finalize())
}

fn evaluation_bytes_digest_v1(
    evaluations: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRlweSourceStatementErrorV1> {
    if evaluations.len() != QPCS_EVALUATION_BYTES_V1 {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidContext);
    }
    let mut hash = Keccak256::new();
    hash.update(EVALUATION_BYTES_DOMAIN_V1);
    hash.update(&[STATEMENT_VERSION_V1]);
    hash.update(
        &u16::try_from(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * ROWS_PER_LIMB_V1)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(evaluations);
    nonzero_digest_v1(hash.finalize())
}

fn public_key_digest_v1(
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    epoch: u64,
    roster_digest: [u8; DIGEST_BYTES_V1],
    public_a_limb_digests: &[[u8; DIGEST_BYTES_V1]],
    public_b_limb_digests: &[[u8; DIGEST_BYTES_V1]],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRlweSourceStatementErrorV1> {
    if epoch == 0
        || roster_digest == [0; DIGEST_BYTES_V1]
        || public_a_limb_digests.len() != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
        || public_b_limb_digests.len() != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
    {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact);
    }
    let mut hash = Keccak256::new();
    hash.update(PUBLIC_KEY_DOMAIN_V1);
    hash.update(&[STATEMENT_VERSION_V1]);
    hash.update(&layout.profile_digest());
    hash.update(&layout.topology_digest());
    hash.update(&layout.release_candidate_digest());
    hash.update(&layout.statement_digest());
    hash.update(&layout.operational_context_digest());
    hash.update(&layout.source_binding_digest());
    hash.update(&epoch.to_be_bytes());
    hash.update(&roster_digest);
    for (limb, ((modulus, public_a), public_b)) in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1
        .into_iter()
        .zip(public_a_limb_digests)
        .zip(public_b_limb_digests)
        .enumerate()
    {
        hash.update(&[u8::try_from(limb)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?]);
        hash.update(&modulus.to_be_bytes());
        hash.update(public_a);
        hash.update(public_b);
    }
    nonzero_digest_v1(hash.finalize())
        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact)
}

#[allow(clippy::too_many_arguments)]
fn nonce_binding_digest_v1(
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    epoch: u64,
    roster_digest: [u8; DIGEST_BYTES_V1],
    public_key_digest: [u8; DIGEST_BYTES_V1],
    position: RecordPositionV1,
    sample_index: u64,
    nonce: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRlweSourceStatementErrorV1> {
    if epoch == 0
        || roster_digest == [0; DIGEST_BYTES_V1]
        || public_key_digest == [0; DIGEST_BYTES_V1]
        || sample_index != u64::from(position.ordinal)
        || nonce.len() != ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_PLAINTEXT_BYTES_V1 as usize
        || nonce.iter().all(|byte| *byte == 0)
    {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidNonce);
    }
    let mut hash = Keccak256::new();
    hash.update(NONCE_BINDING_DOMAIN_V1);
    hash.update(&[STATEMENT_VERSION_V1]);
    hash.update(&layout.profile_digest());
    hash.update(&layout.topology_digest());
    hash.update(&layout.release_candidate_digest());
    hash.update(&layout.source_binding_digest());
    hash.update(&layout.statement_digest());
    hash.update(&layout.operational_context_digest());
    hash.update(&epoch.to_be_bytes());
    hash.update(&roster_digest);
    hash.update(&public_key_digest);
    hash.update(&[
        position.ordinal,
        position.family as u8,
        position.family_index,
        position.family.record_count(),
    ]);
    hash.update(&position.used_slots.to_be_bytes());
    hash.update(&sample_index.to_be_bytes());
    hash.update(nonce);
    nonzero_digest_v1(hash.finalize())
        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidNonce)
}

fn public_record_digest_v1(
    public_key_digest: [u8; DIGEST_BYTES_V1],
    position: RecordPositionV1,
    sample_index: u64,
    nonce_binding_digest: [u8; DIGEST_BYTES_V1],
    c0_limb_digests: &[[u8; DIGEST_BYTES_V1]],
    c1_limb_digests: &[[u8; DIGEST_BYTES_V1]],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRlweSourceStatementErrorV1> {
    if public_key_digest == [0; DIGEST_BYTES_V1]
        || nonce_binding_digest == [0; DIGEST_BYTES_V1]
        || sample_index != u64::from(position.ordinal)
        || c0_limb_digests.len() != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
        || c1_limb_digests.len() != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
    {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact);
    }
    let mut hash = Keccak256::new();
    hash.update(PUBLIC_RECORD_DOMAIN_V1);
    hash.update(&[STATEMENT_VERSION_V1]);
    hash.update(&public_key_digest);
    hash.update(&[
        position.ordinal,
        position.family as u8,
        position.family_index,
        position.family.record_count(),
    ]);
    hash.update(&position.used_slots.to_be_bytes());
    hash.update(&sample_index.to_be_bytes());
    hash.update(&nonce_binding_digest);
    for (limb, ((modulus, c0), c1)) in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1
        .into_iter()
        .zip(c0_limb_digests)
        .zip(c1_limb_digests)
        .enumerate()
    {
        hash.update(&[u8::try_from(limb)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?]);
        hash.update(&modulus.to_be_bytes());
        hash.update(c0);
        hash.update(c1);
    }
    nonzero_digest_v1(hash.finalize())
        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact)
}

fn public_bundle_digest_v1(
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    epoch: u64,
    roster_digest: [u8; DIGEST_BYTES_V1],
    public_key_digest: [u8; DIGEST_BYTES_V1],
    records: &[RnsNativePublicRecordMetadataV1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRlweSourceStatementErrorV1> {
    if epoch == 0
        || roster_digest == [0; DIGEST_BYTES_V1]
        || public_key_digest == [0; DIGEST_BYTES_V1]
        || records.len() != OPENING_COUNT_V1
    {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact);
    }
    let mut hash = Keccak256::new();
    hash.update(PUBLIC_BUNDLE_DOMAIN_V1);
    hash.update(&[STATEMENT_VERSION_V1]);
    hash.update(&layout.profile_digest());
    hash.update(&layout.topology_digest());
    hash.update(&layout.release_candidate_digest());
    hash.update(&layout.source_binding_digest());
    hash.update(&layout.statement_digest());
    hash.update(&layout.operational_context_digest());
    hash.update(&epoch.to_be_bytes());
    hash.update(&roster_digest);
    hash.update(&public_key_digest);
    for (ordinal, record) in records.iter().enumerate() {
        let expected = record_position_v1(ordinal)
            .ok_or(RnsNativeRlweSourceStatementErrorV1::InvalidSourceOrder)?;
        if record.ordinal != expected.ordinal
            || record.family != expected.family
            || record.family_index != expected.family_index
            || record.sample_index != u64::from(expected.ordinal)
        {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidSourceOrder);
        }
        hash.update(&[record.ordinal, record.family as u8, record.family_index]);
        hash.update(&record.sample_index.to_be_bytes());
        hash.update(&record.nonce_binding_digest);
        hash.update(&record.record_digest);
    }
    nonzero_digest_v1(hash.finalize())
        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ValidatedPublicArtifactV1 {
    public_key_digest: [u8; DIGEST_BYTES_V1],
    public_bundle_digest: [u8; DIGEST_BYTES_V1],
}

fn validate_public_artifact_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    public: RnsNativePublicArtifactViewV1<'_>,
) -> Result<ValidatedPublicArtifactV1, RnsNativeRlweSourceStatementErrorV1> {
    if public.governed_roster_digest != transcript.governed_roster_digest()
        || public.public_bundle_digest != transcript.public_ciphertext_digest()
    {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact);
    }
    validate_public_artifact_without_transcript_v1(layout, public)
}

fn validate_public_artifact_without_transcript_v1(
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    public: RnsNativePublicArtifactViewV1<'_>,
) -> Result<ValidatedPublicArtifactV1, RnsNativeRlweSourceStatementErrorV1> {
    if public.epoch == 0
        || public.governed_roster_digest == [0; DIGEST_BYTES_V1]
        || public.public_bundle_digest == [0; DIGEST_BYTES_V1]
        || public.public_a_limb_digests.len() != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
        || public.public_b_limb_digests.len() != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
        || public.ciphertext_c0_limb_digests.len() != PUBLIC_LIMB_DIGEST_COUNT_V1
        || public.ciphertext_c1_limb_digests.len() != PUBLIC_LIMB_DIGEST_COUNT_V1
        || public.records.len() != OPENING_COUNT_V1
    {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact);
    }
    let public_key_digest = public_key_digest_v1(
        layout,
        public.epoch,
        public.governed_roster_digest,
        public.public_a_limb_digests,
        public.public_b_limb_digests,
    )?;
    let mut registry = DigestRegistryV1::with_capacity_v1(
        3 + 2 * ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
            + 2 * PUBLIC_LIMB_DIGEST_COUNT_V1
            + 2 * OPENING_COUNT_V1,
    )?;
    registry.insert_v1(public.governed_roster_digest)?;
    registry.insert_v1(public_key_digest)?;
    for digest in public
        .public_a_limb_digests
        .iter()
        .chain(public.public_b_limb_digests)
        .chain(public.ciphertext_c0_limb_digests)
        .chain(public.ciphertext_c1_limb_digests)
    {
        registry.insert_v1(*digest)?;
    }
    for (ordinal, record) in public.records.iter().enumerate() {
        let position = record_position_v1(ordinal)
            .ok_or(RnsNativeRlweSourceStatementErrorV1::InvalidSourceOrder)?;
        if record.ordinal != position.ordinal
            || record.family != position.family
            || record.family_index != position.family_index
            || record.sample_index != u64::from(position.ordinal)
        {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidSourceOrder);
        }
        let start = ordinal
            .checked_mul(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1)
            .ok_or(RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?;
        let end = start
            .checked_add(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1)
            .ok_or(RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?;
        let expected_record_digest = public_record_digest_v1(
            public_key_digest,
            position,
            record.sample_index,
            record.nonce_binding_digest,
            public
                .ciphertext_c0_limb_digests
                .get(start..end)
                .ok_or(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact)?,
            public
                .ciphertext_c1_limb_digests
                .get(start..end)
                .ok_or(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact)?,
        )?;
        if record.record_digest != expected_record_digest {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact);
        }
        registry.insert_v1(record.nonce_binding_digest)?;
        registry.insert_v1(record.record_digest)?;
    }
    let expected_bundle = public_bundle_digest_v1(
        layout,
        public.epoch,
        public.governed_roster_digest,
        public_key_digest,
        public.records,
    )?;
    if public.public_bundle_digest != expected_bundle {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact);
    }
    registry.insert_v1(expected_bundle)?;
    Ok(ValidatedPublicArtifactV1 {
        public_key_digest,
        public_bundle_digest: expected_bundle,
    })
}

/// Revalidate public facts without constructing a transcript or returning a
/// reusable borrowed view.  This is a pure hash/facts check; it grants no
/// source, preflight, verification, readiness, or release authority.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_rns_native_pre_transcript_public_facts_v1(
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    epoch: u64,
    governed_roster_digest: [u8; DIGEST_BYTES_V1],
    public_a_limb_digests: &[[u8; DIGEST_BYTES_V1]],
    public_b_limb_digests: &[[u8; DIGEST_BYTES_V1]],
    ciphertext_c0_limb_digests: &[[u8; DIGEST_BYTES_V1]],
    ciphertext_c1_limb_digests: &[[u8; DIGEST_BYTES_V1]],
    records: &[RnsNativePublicRecordMetadataV1],
    public_key_digest: [u8; DIGEST_BYTES_V1],
    public_bundle_digest: [u8; DIGEST_BYTES_V1],
) -> Result<(), RnsNativeRlweSourceStatementErrorV1> {
    layout
        .validate()
        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidContext)?;
    let public = RnsNativePublicArtifactViewV1::new(
        epoch,
        governed_roster_digest,
        public_a_limb_digests,
        public_b_limb_digests,
        ciphertext_c0_limb_digests,
        ciphertext_c1_limb_digests,
        records,
        public_bundle_digest,
    );
    let validated = validate_public_artifact_without_transcript_v1(layout, public)?;
    if validated.public_key_digest != public_key_digest
        || validated.public_bundle_digest != public_bundle_digest
    {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact);
    }
    Ok(())
}

/// Perform the one bounded source/hash pass that derives the exact 43 record
/// facts and two aggregate digests.  The caller must keep the same snapshot
/// and the complete artifact inventory by value; this narrow helper returns no
/// transcript constructor, public-context authority, or retained borrow.
#[allow(clippy::too_many_arguments)]
pub(super) fn derive_rns_native_pre_transcript_record_facts_v1<S>(
    snapshot: &mut S,
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    epoch: u64,
    governed_roster_digest: [u8; DIGEST_BYTES_V1],
    public_a_limb_digests: &[[u8; DIGEST_BYTES_V1]],
    public_b_limb_digests: &[[u8; DIGEST_BYTES_V1]],
    ciphertext_c0_limb_digests: &[[u8; DIGEST_BYTES_V1]],
    ciphertext_c1_limb_digests: &[[u8; DIGEST_BYTES_V1]],
) -> Result<
    (
        [RnsNativePublicRecordMetadataV1; 43],
        [u8; DIGEST_BYTES_V1],
        [u8; DIGEST_BYTES_V1],
    ),
    RnsNativeRlweSourceStatementErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    layout
        .validate()
        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidContext)?;
    if snapshot.layout() != layout
        || epoch == 0
        || governed_roster_digest == [0; DIGEST_BYTES_V1]
        || public_a_limb_digests.len() != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
        || public_b_limb_digests.len() != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
        || ciphertext_c0_limb_digests.len() != PUBLIC_LIMB_DIGEST_COUNT_V1
        || ciphertext_c1_limb_digests.len() != PUBLIC_LIMB_DIGEST_COUNT_V1
    {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact);
    }

    let public_key_digest = public_key_digest_v1(
        layout,
        epoch,
        governed_roster_digest,
        public_a_limb_digests,
        public_b_limb_digests,
    )?;
    // Reject every caller-independent public alias before the 315,622,752-byte
    // source pass. Nonce/record/bundle identities join this same exact
    // 3,609-entry registry as they are derived.
    let mut registry =
        DigestRegistryV1::with_capacity_v1(RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ALIAS_DIGESTS_V1)?;
    registry.insert_v1(governed_roster_digest)?;
    registry.insert_v1(public_key_digest)?;
    for digest in public_a_limb_digests
        .iter()
        .chain(public_b_limb_digests.iter())
        .chain(ciphertext_c0_limb_digests.iter())
        .chain(ciphertext_c1_limb_digests.iter())
    {
        registry.insert_v1(*digest)?;
    }

    let empty_record = RnsNativePublicRecordMetadataV1::new(
        0,
        ZkAmsMkheRnsNativeFamilyV1::X,
        0,
        0,
        [0; DIGEST_BYTES_V1],
        [0; DIGEST_BYTES_V1],
    );
    let mut records = [empty_record; OPENING_COUNT_V1];
    for ordinal in 0..OPENING_COUNT_V1 {
        let position = record_position_v1(ordinal)
            .ok_or(RnsNativeRlweSourceStatementErrorV1::InvalidSourceOrder)?;
        let mut ephemeral_nonzero = false;
        for component in SOURCE_COMPONENT_ORDER_V1 {
            for block in 0..component.block_count() {
                let slot = absolute_main_slot_v1(ordinal, component, block)?;
                let chunk = snapshot
                    .read_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, slot)
                    .map_err(|_| RnsNativeRlweSourceStatementErrorV1::SourceUnavailable)?;
                if chunk.arena() != ZkAmsMkheRnsNativeSourceArenaV1::Main {
                    return Err(RnsNativeRlweSourceStatementErrorV1::InvalidSourceOrder);
                }
                match component {
                    SourceComponentV1::CanonicalPlaintext => {
                        validate_canonical_plaintext_chunk_v1(chunk.as_slice())?;
                    }
                    SourceComponentV1::Ephemeral
                    | SourceComponentV1::ErrorZero
                    | SourceComponentV1::ErrorOne => {
                        validate_signed_chunk_v1(
                            chunk.as_slice(),
                            component,
                            &mut ephemeral_nonzero,
                        )?;
                    }
                }
            }
        }
        if !ephemeral_nonzero {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidEphemeral);
        }
        let nonce_chunk = snapshot
            .read_slot(
                ZkAmsMkheRnsNativeSourceArenaV1::Nonce,
                u64::try_from(ordinal)
                    .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
            )
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::SourceUnavailable)?;
        if nonce_chunk.arena() != ZkAmsMkheRnsNativeSourceArenaV1::Nonce
            || nonce_chunk.as_slice().len()
                != ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_PLAINTEXT_BYTES_V1 as usize
        {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidSourceEncoding);
        }
        let sample_index = u64::from(position.ordinal);
        let nonce_binding_digest = nonce_binding_digest_v1(
            layout,
            epoch,
            governed_roster_digest,
            public_key_digest,
            position,
            sample_index,
            nonce_chunk.as_slice(),
        )?;
        let start = ordinal
            .checked_mul(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1)
            .ok_or(RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?;
        let end = start
            .checked_add(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1)
            .ok_or(RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?;
        let record_digest = public_record_digest_v1(
            public_key_digest,
            position,
            sample_index,
            nonce_binding_digest,
            ciphertext_c0_limb_digests
                .get(start..end)
                .ok_or(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact)?,
            ciphertext_c1_limb_digests
                .get(start..end)
                .ok_or(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact)?,
        )?;
        registry.insert_v1(nonce_binding_digest)?;
        registry.insert_v1(record_digest)?;
        records[ordinal] = RnsNativePublicRecordMetadataV1::new(
            position.ordinal,
            position.family,
            position.family_index,
            sample_index,
            nonce_binding_digest,
            record_digest,
        );
    }
    let public_bundle_digest = public_bundle_digest_v1(
        layout,
        epoch,
        governed_roster_digest,
        public_key_digest,
        &records,
    )?;
    registry.insert_v1(public_bundle_digest)?;
    drop(registry);
    Ok((records, public_key_digest, public_bundle_digest))
}

fn map_challenge_candidate_v1(raw: u64, modulus: u64, used: &[u64]) -> Option<u64> {
    if modulus < 3 {
        return None;
    }
    let rejection_bound = u64::MAX - (u64::MAX % modulus);
    if raw >= rejection_bound {
        return None;
    }
    let candidate = raw % modulus;
    if candidate == 0 || used.contains(&candidate) {
        return None;
    }
    Some(candidate)
}

#[allow(clippy::too_many_arguments)]
fn derive_aggregation_challenge_coordinate_v1(
    aggregation_seed: [u8; DIGEST_BYTES_V1],
    parameter_digest: [u8; DIGEST_BYTES_V1],
    formula_digest: [u8; DIGEST_BYTES_V1],
    mapping_digest: [u8; DIGEST_BYTES_V1],
    limb: usize,
    repetition: usize,
    role: u8,
    modulus: u64,
    used: &[u64],
) -> Result<u64, RnsNativeRlweSourceStatementErrorV1> {
    if aggregation_seed == [0; DIGEST_BYTES_V1]
        || parameter_digest == [0; DIGEST_BYTES_V1]
        || formula_digest == [0; DIGEST_BYTES_V1]
        || mapping_digest == [0; DIGEST_BYTES_V1]
        || limb >= ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
        || repetition >= REPETITION_COUNT_V1
        || role > 1
    {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidChallenge);
    }
    for attempt in 0..MAX_CHALLENGE_ATTEMPTS_V1 {
        let mut hash = Keccak256::new();
        hash.update(AGGREGATION_CHALLENGE_DOMAIN_V1);
        hash.update(&[STATEMENT_VERSION_V1]);
        hash.update(&parameter_digest);
        hash.update(&aggregation_seed);
        hash.update(&formula_digest);
        hash.update(&mapping_digest);
        hash.update(&[
            u8::try_from(limb)
                .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
            u8::try_from(repetition)
                .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
            role,
        ]);
        hash.update(&modulus.to_be_bytes());
        hash.update(&attempt.to_be_bytes());
        let digest = hash.finalize();
        let raw = u64::from_be_bytes(
            digest[..8]
                .try_into()
                .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidChallenge)?,
        );
        if let Some(candidate) = map_challenge_candidate_v1(raw, modulus, used) {
            return Ok(candidate);
        }
    }
    Err(RnsNativeRlweSourceStatementErrorV1::InvalidChallenge)
}

fn derive_aggregation_challenges_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    parameter_digest: [u8; DIGEST_BYTES_V1],
    formula_digest: [u8; DIGEST_BYTES_V1],
    mapping_digest: [u8; DIGEST_BYTES_V1],
) -> Result<
    [[AggregationChallengeV1; REPETITION_COUNT_V1]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
    RnsNativeRlweSourceStatementErrorV1,
> {
    let seed = transcript.rns_aggregation_challenge_seed();
    let empty = AggregationChallengeV1 { gamma: 0, beta: 0 };
    let mut challenges = [[empty; REPETITION_COUNT_V1]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1];
    let mut prior_pairs = [(0_u64, 0_u64); ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * REPETITION_COUNT_V1];
    let mut prior_pair_count = 0_usize;
    for (limb, modulus) in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1.into_iter().enumerate() {
        let mut used = [0_u64; ROWS_PER_LIMB_V1];
        let mut used_len = 0_usize;
        for (repetition, challenge) in challenges[limb].iter_mut().enumerate() {
            let gamma = derive_aggregation_challenge_coordinate_v1(
                seed,
                parameter_digest,
                formula_digest,
                mapping_digest,
                limb,
                repetition,
                0,
                modulus,
                &used[..used_len],
            )?;
            used[used_len] = gamma;
            used_len += 1;
            let beta = derive_aggregation_challenge_coordinate_v1(
                seed,
                parameter_digest,
                formula_digest,
                mapping_digest,
                limb,
                repetition,
                1,
                modulus,
                &used[..used_len],
            )?;
            used[used_len] = beta;
            used_len += 1;
            if prior_pairs[..prior_pair_count].contains(&(gamma, beta)) {
                return Err(RnsNativeRlweSourceStatementErrorV1::InvalidChallenge);
            }
            prior_pairs[prior_pair_count] = (gamma, beta);
            prior_pair_count += 1;
            *challenge = AggregationChallengeV1 { gamma, beta };
        }
    }
    Ok(challenges)
}

#[allow(clippy::too_many_arguments)]
fn aggregation_schedule_digest_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    parameter_digest: [u8; DIGEST_BYTES_V1],
    formula_digest: [u8; DIGEST_BYTES_V1],
    mapping_digest: [u8; DIGEST_BYTES_V1],
    opening_bundle_digest: [u8; DIGEST_BYTES_V1],
    equation_bundle_digest: [u8; DIGEST_BYTES_V1],
    limb_bundle_digest: [u8; DIGEST_BYTES_V1],
    evaluation_binding_digest: [u8; DIGEST_BYTES_V1],
    evaluations_digest: [u8; DIGEST_BYTES_V1],
    public_bundle_digest: [u8; DIGEST_BYTES_V1],
    challenges: &[[AggregationChallengeV1; REPETITION_COUNT_V1]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRlweSourceStatementErrorV1> {
    let identities = [
        parameter_digest,
        formula_digest,
        mapping_digest,
        opening_bundle_digest,
        equation_bundle_digest,
        limb_bundle_digest,
        evaluation_binding_digest,
        evaluations_digest,
        public_bundle_digest,
    ];
    if identities.contains(&[0; DIGEST_BYTES_V1]) {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidContext);
    }
    let mut hash = Keccak256::new();
    hash.update(AGGREGATION_SCHEDULE_DOMAIN_V1);
    hash.update(&[STATEMENT_VERSION_V1]);
    hash.update(&transcript.rns_aggregation_challenge_seed());
    hash.update(&transcript.transcript_digest());
    for identity in identities {
        hash.update(&identity);
    }
    hash.update(&[
        u8::try_from(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
        u8::try_from(REPETITION_COUNT_V1)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
        u8::try_from(ROWS_PER_REPETITION_V1)
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
    ]);
    for (limb, (modulus, repetitions)) in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1
        .into_iter()
        .zip(challenges)
        .enumerate()
    {
        for (repetition, challenge) in repetitions.iter().enumerate() {
            hash.update(&[
                u8::try_from(limb)
                    .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
                u8::try_from(repetition)
                    .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
            ]);
            hash.update(&modulus.to_be_bytes());
            hash.update(&challenge.gamma.to_be_bytes());
            hash.update(&challenge.beta.to_be_bytes());
            for role in QPCS_ROW_ROLES_V1 {
                hash.update(&[role as u8]);
            }
        }
    }
    nonzero_digest_v1(hash.finalize())
}

#[derive(Clone, Copy)]
struct QpcsBindingsV1<'a> {
    relation_schedule_present: bool,
    parameter_digest: [u8; DIGEST_BYTES_V1],
    transcript_digest: [u8; DIGEST_BYTES_V1],
    query_seed: [u8; DIGEST_BYTES_V1],
    section_binding_digest: [u8; DIGEST_BYTES_V1],
    fri_schedule_digest: [u8; DIGEST_BYTES_V1],
    evaluations: &'a [u8],
    evaluation_binding_digest: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    residual: &'a [u8],
}

impl<'a> QpcsBindingsV1<'a> {
    fn from_stage_v1(stage: &RnsNativeQpcsFriCompleteStageV1<'a>) -> Self {
        Self {
            relation_schedule_present: stage.has_relation_schedule_v1(),
            parameter_digest: stage.parameter_digest(),
            transcript_digest: stage.transcript_digest(),
            query_seed: stage.query_seed(),
            section_binding_digest: stage.section_binding_digest(),
            fri_schedule_digest: stage.schedule_digest(),
            evaluations: stage.evaluations(),
            evaluation_binding_digest: stage.evaluation_binding_digest(),
            residual_digest: stage.residual_digest(),
            residual: stage.rlwe_source_residual(),
        }
    }

    fn validate_v1(
        self,
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    ) -> Result<(), RnsNativeRlweSourceStatementErrorV1> {
        let identities = [
            self.parameter_digest,
            self.transcript_digest,
            self.query_seed,
            self.section_binding_digest,
            self.fri_schedule_digest,
            self.evaluation_binding_digest,
            self.residual_digest,
        ];
        if !self.relation_schedule_present
            || identities.contains(&[0; DIGEST_BYTES_V1])
            || identities
                .iter()
                .enumerate()
                .any(|(index, digest)| identities[index + 1..].contains(digest))
            || self.transcript_digest != transcript.transcript_digest()
            || self.query_seed != transcript.qpcs_query_challenge_seed()
            || self.evaluations.len() != QPCS_EVALUATION_BYTES_V1
            || self.residual.is_empty()
            || self.residual.len() > RNS_NATIVE_RLWE_SOURCE_RESIDUAL_MAX_BYTES_V1
        {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidContext);
        }
        Ok(())
    }
}

fn validate_context_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    qpcs: QpcsBindingsV1<'_>,
) -> Result<(), RnsNativeRlweSourceStatementErrorV1> {
    layout
        .validate()
        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidContext)?;
    receipt
        .validate(layout)
        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidContext)?;
    let manifest = zk_ams_mkhe_rns_native_profile_manifest_v1()
        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidContext)?;
    let topology = zk_ams_mkhe_rns_native_topology_v1()
        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidContext)?;
    let release_candidate = zk_ams_mkhe_rns_native_release_candidate_digest_v1()
        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidContext)?;
    if transcript.profile_manifest_digest() != manifest.manifest_digest
        || transcript.profile_digest() != manifest.profile_digest
        || transcript.topology_digest() != topology.topology_digest
        || transcript.release_candidate_digest() != release_candidate
        || transcript.profile_digest() != layout.profile_digest()
        || transcript.topology_digest() != layout.topology_digest()
        || transcript.release_candidate_digest() != layout.release_candidate_digest()
        || transcript.statement_digest() != layout.statement_digest()
        || transcript.operational_context_digest() != layout.operational_context_digest()
        || transcript.source_binding_digest() != layout.source_binding_digest()
        || transcript.main_snapshot_digest() != receipt.main_snapshot_digest
        || transcript.nonce_snapshot_digest() != receipt.nonce_snapshot_digest
        || transcript.source_receipt_digest() != receipt.receipt_digest
        || receipt.source_binding_digest != layout.source_binding_digest()
    {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidContext);
    }
    qpcs.validate_v1(transcript)
}

#[allow(clippy::too_many_arguments)]
fn validate_global_input_aliases_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    public: RnsNativePublicArtifactViewV1<'_>,
    equation_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
    limb_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
    qpcs: QpcsBindingsV1<'_>,
) -> Result<(), RnsNativeRlweSourceStatementErrorV1> {
    if equation_commitment_digests.len() != EQUATION_COUNT_V1
        || limb_commitment_digests.len() != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
    {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidContext);
    }
    let capacity = 20
        + 2 * OPENING_COUNT_V1
        + EQUATION_COUNT_V1
        + ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
        + 2 * ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
        + 2 * PUBLIC_LIMB_DIGEST_COUNT_V1
        + 2 * OPENING_COUNT_V1;
    let mut registry = DigestRegistryV1::with_capacity_v1(capacity)?;
    for digest in [
        transcript.profile_manifest_digest(),
        transcript.profile_digest(),
        transcript.topology_digest(),
        transcript.release_candidate_digest(),
        transcript.statement_digest(),
        transcript.operational_context_digest(),
        transcript.source_binding_digest(),
        transcript.main_snapshot_digest(),
        transcript.nonce_snapshot_digest(),
        transcript.source_receipt_digest(),
        transcript.governed_roster_digest(),
        transcript.public_ciphertext_digest(),
        transcript.rns_aggregation_challenge_seed(),
        qpcs.parameter_digest,
        qpcs.transcript_digest,
        qpcs.query_seed,
        qpcs.section_binding_digest,
        qpcs.fri_schedule_digest,
        qpcs.evaluation_binding_digest,
        qpcs.residual_digest,
    ] {
        registry.insert_v1(digest)?;
    }
    for opening in transcript.opening_commitments() {
        registry.insert_v1(opening.source_commitment_digest())?;
        registry.insert_v1(opening.hyrax_commitment_digest())?;
    }
    for digest in equation_commitment_digests
        .iter()
        .chain(limb_commitment_digests)
        .chain(public.public_a_limb_digests)
        .chain(public.public_b_limb_digests)
        .chain(public.ciphertext_c0_limb_digests)
        .chain(public.ciphertext_c1_limb_digests)
    {
        registry.insert_v1(*digest)?;
    }
    for record in public.records {
        registry.insert_v1(record.nonce_binding_digest)?;
        registry.insert_v1(record.record_digest)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct DerivedStatementV1 {
    epoch: u64,
    public_key_digest: [u8; DIGEST_BYTES_V1],
    public_bundle_digest: [u8; DIGEST_BYTES_V1],
    formula_digest: [u8; DIGEST_BYTES_V1],
    mapping_digest: [u8; DIGEST_BYTES_V1],
    opening_bundle_digest: [u8; DIGEST_BYTES_V1],
    equation_bundle_digest: [u8; DIGEST_BYTES_V1],
    limb_bundle_digest: [u8; DIGEST_BYTES_V1],
    aggregation_schedule_digest: [u8; DIGEST_BYTES_V1],
    preflight_statement_digest: [u8; DIGEST_BYTES_V1],
    challenges: [[AggregationChallengeV1; REPETITION_COUNT_V1]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
}

fn preflight_statement_digest_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    public_key_digest: [u8; DIGEST_BYTES_V1],
    public_bundle_digest: [u8; DIGEST_BYTES_V1],
    formula_digest: [u8; DIGEST_BYTES_V1],
    mapping_digest: [u8; DIGEST_BYTES_V1],
    aggregation_schedule_digest: [u8; DIGEST_BYTES_V1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRlweSourceStatementErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(PUBLIC_STATEMENT_DOMAIN_V1);
    hash.update(&[STATEMENT_VERSION_V1]);
    hash.update(&transcript.statement_digest());
    hash.update(&transcript.operational_context_digest());
    hash.update(&transcript.source_binding_digest());
    hash.update(&receipt.main_snapshot_digest);
    hash.update(&receipt.nonce_snapshot_digest);
    hash.update(&receipt.receipt_digest);
    hash.update(&transcript.governed_roster_digest());
    hash.update(&public_key_digest);
    hash.update(&public_bundle_digest);
    hash.update(&formula_digest);
    hash.update(&mapping_digest);
    hash.update(&aggregation_schedule_digest);
    nonzero_digest_v1(hash.finalize())
}

#[allow(clippy::too_many_arguments)]
fn derive_statement_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    public: RnsNativePublicArtifactViewV1<'_>,
    equation_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
    limb_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
    qpcs: QpcsBindingsV1<'_>,
) -> Result<DerivedStatementV1, RnsNativeRlweSourceStatementErrorV1> {
    validate_context_v1(transcript, layout, receipt, qpcs)?;
    let validated_public = validate_public_artifact_v1(transcript, layout, public)?;
    validate_global_input_aliases_v1(
        transcript,
        public,
        equation_commitment_digests,
        limb_commitment_digests,
        qpcs,
    )?;
    let formula_digest = rlwe_formula_digest_v1()?;
    let mapping_digest = record_mapping_digest_v1()?;
    let opening_bundle_digest = opening_bundle_digest_v1(transcript)?;
    let equation_bundle_digest = equation_bundle_digest_v1(equation_commitment_digests)?;
    let limb_bundle_digest = limb_bundle_digest_v1(limb_commitment_digests)?;
    let evaluations_digest = evaluation_bytes_digest_v1(qpcs.evaluations)?;
    let challenges = derive_aggregation_challenges_v1(
        transcript,
        qpcs.parameter_digest,
        formula_digest,
        mapping_digest,
    )?;
    let aggregation_schedule_digest = aggregation_schedule_digest_v1(
        transcript,
        qpcs.parameter_digest,
        formula_digest,
        mapping_digest,
        opening_bundle_digest,
        equation_bundle_digest,
        limb_bundle_digest,
        qpcs.evaluation_binding_digest,
        evaluations_digest,
        validated_public.public_bundle_digest,
        &challenges,
    )?;
    let preflight_statement_digest = preflight_statement_digest_v1(
        transcript,
        receipt,
        validated_public.public_key_digest,
        validated_public.public_bundle_digest,
        formula_digest,
        mapping_digest,
        aggregation_schedule_digest,
    )?;
    Ok(DerivedStatementV1 {
        epoch: public.epoch,
        public_key_digest: validated_public.public_key_digest,
        public_bundle_digest: validated_public.public_bundle_digest,
        formula_digest,
        mapping_digest,
        opening_bundle_digest,
        equation_bundle_digest,
        limb_bundle_digest,
        aggregation_schedule_digest,
        preflight_statement_digest,
        challenges,
    })
}

fn expected_anchor_core_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    qpcs: QpcsBindingsV1<'_>,
    derived: DerivedStatementV1,
    downstream: &[u8],
) -> Result<[[u8; DIGEST_BYTES_V1]; ANCHOR_CORE_DIGESTS_V1], RnsNativeRlweSourceStatementErrorV1> {
    let mut core = [[0; DIGEST_BYTES_V1]; ANCHOR_CORE_DIGESTS_V1];
    core[CORE_QPCS_PARAMETER_V1] = qpcs.parameter_digest;
    core[CORE_TRANSCRIPT_V1] = transcript.transcript_digest();
    core[CORE_QUERY_SEED_V1] = qpcs.query_seed;
    core[CORE_QPCS_SECTION_V1] = qpcs.section_binding_digest;
    core[CORE_FRI_SCHEDULE_V1] = qpcs.fri_schedule_digest;
    core[CORE_PROFILE_V1] = layout.profile_digest();
    core[CORE_TOPOLOGY_V1] = layout.topology_digest();
    core[CORE_RELEASE_CANDIDATE_V1] = layout.release_candidate_digest();
    core[CORE_SOURCE_BINDING_V1] = layout.source_binding_digest();
    core[CORE_MAIN_SNAPSHOT_V1] = receipt.main_snapshot_digest;
    core[CORE_NONCE_SNAPSHOT_V1] = receipt.nonce_snapshot_digest;
    core[CORE_SOURCE_RECEIPT_V1] = receipt.receipt_digest;
    core[CORE_STATEMENT_V1] = layout.statement_digest();
    core[CORE_OPERATIONAL_V1] = layout.operational_context_digest();
    core[CORE_ROSTER_V1] = transcript.governed_roster_digest();
    core[CORE_PUBLIC_BUNDLE_V1] = derived.public_bundle_digest;
    core[CORE_OPENING_BUNDLE_V1] = derived.opening_bundle_digest;
    core[CORE_FORMULA_V1] = derived.formula_digest;
    core[CORE_MAPPING_V1] = derived.mapping_digest;
    core[CORE_EQUATION_BUNDLE_V1] = derived.equation_bundle_digest;
    core[CORE_LIMB_BUNDLE_V1] = derived.limb_bundle_digest;
    core[CORE_AGGREGATION_SCHEDULE_V1] = derived.aggregation_schedule_digest;
    core[CORE_DOWNSTREAM_V1] = downstream_digest_v1(downstream)?;
    let anchor = ResidualAnchorV1::from_parts_v1(derived.epoch, core, downstream)?;
    Ok(anchor.core_digests)
}

fn absolute_main_slot_v1(
    record: usize,
    component: SourceComponentV1,
    block: usize,
) -> Result<u64, RnsNativeRlweSourceStatementErrorV1> {
    if record >= OPENING_COUNT_V1 || block >= component.block_count() {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidSourceOrder);
    }
    let slot = record
        .checked_mul(MAIN_BLOCKS_PER_RECORD_V1)
        .and_then(|base| base.checked_add(component.first_block()))
        .and_then(|base| base.checked_add(block))
        .ok_or(RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?;
    u64::try_from(slot).map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)
}

fn validate_canonical_plaintext_chunk_v1(
    bytes: &[u8],
) -> Result<(), RnsNativeRlweSourceStatementErrorV1> {
    if bytes.len() != ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1 as usize {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidSourceEncoding);
    }
    let mut coefficients = bytes.chunks_exact(CANONICAL_COEFFICIENT_BYTES_V1);
    for coefficient in &mut coefficients {
        if coefficient >= VEGA_T256_SCALAR_MODULUS_BE_V1.as_slice() {
            return Err(RnsNativeRlweSourceStatementErrorV1::NonCanonicalPlaintext);
        }
    }
    if !coefficients.remainder().is_empty() {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidSourceEncoding);
    }
    Ok(())
}

fn validate_signed_chunk_v1(
    bytes: &[u8],
    component: SourceComponentV1,
    ephemeral_nonzero: &mut bool,
) -> Result<(), RnsNativeRlweSourceStatementErrorV1> {
    if bytes.len() != ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1 as usize
        || component == SourceComponentV1::CanonicalPlaintext
    {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidSourceEncoding);
    }
    let mut coefficients = bytes.chunks_exact(SIGNED_COEFFICIENT_BYTES_V1);
    for encoded in &mut coefficients {
        let value = i64::from_be_bytes(
            encoded
                .try_into()
                .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidSourceEncoding)?,
        );
        match component {
            SourceComponentV1::Ephemeral => {
                if !(-1..=1).contains(&value) {
                    return Err(RnsNativeRlweSourceStatementErrorV1::InvalidEphemeral);
                }
                *ephemeral_nonzero |= value != 0;
            }
            SourceComponentV1::ErrorZero | SourceComponentV1::ErrorOne => {
                if value.unsigned_abs() > 2 {
                    return Err(RnsNativeRlweSourceStatementErrorV1::InvalidError);
                }
            }
            SourceComponentV1::CanonicalPlaintext => {
                return Err(RnsNativeRlweSourceStatementErrorV1::InvalidSourceEncoding);
            }
        }
    }
    if !coefficients.remainder().is_empty() {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidSourceEncoding);
    }
    Ok(())
}

fn validate_source_snapshot_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
    snapshot: &mut S,
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    public: RnsNativePublicArtifactViewV1<'_>,
    public_key_digest: [u8; DIGEST_BYTES_V1],
) -> Result<(), RnsNativeRlweSourceStatementErrorV1> {
    if snapshot.layout() != layout {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidContext);
    }
    receipt
        .validate(layout)
        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidContext)?;
    let live_receipt = snapshot
        .structural_receipt()
        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::SourceUnavailable)?;
    if live_receipt != receipt {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidContext);
    }
    for ordinal in 0..OPENING_COUNT_V1 {
        let position = record_position_v1(ordinal)
            .ok_or(RnsNativeRlweSourceStatementErrorV1::InvalidSourceOrder)?;
        let record = *public
            .records
            .get(ordinal)
            .ok_or(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact)?;
        if record.ordinal != position.ordinal
            || record.family != position.family
            || record.family_index != position.family_index
            || record.sample_index != u64::from(position.ordinal)
        {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidSourceOrder);
        }
        let mut ephemeral_nonzero = false;
        for component in SOURCE_COMPONENT_ORDER_V1 {
            for block in 0..component.block_count() {
                let slot = absolute_main_slot_v1(ordinal, component, block)?;
                let chunk = snapshot
                    .read_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, slot)
                    .map_err(|_| RnsNativeRlweSourceStatementErrorV1::SourceUnavailable)?;
                if chunk.arena() != ZkAmsMkheRnsNativeSourceArenaV1::Main {
                    return Err(RnsNativeRlweSourceStatementErrorV1::InvalidSourceOrder);
                }
                match component {
                    SourceComponentV1::CanonicalPlaintext => {
                        validate_canonical_plaintext_chunk_v1(chunk.as_slice())?;
                    }
                    SourceComponentV1::Ephemeral
                    | SourceComponentV1::ErrorZero
                    | SourceComponentV1::ErrorOne => {
                        validate_signed_chunk_v1(
                            chunk.as_slice(),
                            component,
                            &mut ephemeral_nonzero,
                        )?;
                    }
                }
            }
        }
        if !ephemeral_nonzero {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidEphemeral);
        }
        let nonce_chunk = snapshot
            .read_slot(
                ZkAmsMkheRnsNativeSourceArenaV1::Nonce,
                u64::try_from(ordinal)
                    .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?,
            )
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::SourceUnavailable)?;
        if nonce_chunk.arena() != ZkAmsMkheRnsNativeSourceArenaV1::Nonce
            || nonce_chunk.as_slice().len()
                != ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_PLAINTEXT_BYTES_V1 as usize
        {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidSourceEncoding);
        }
        let expected_nonce_binding = nonce_binding_digest_v1(
            layout,
            public.epoch,
            public.governed_roster_digest,
            public_key_digest,
            position,
            record.sample_index,
            nonce_chunk.as_slice(),
        )?;
        if record.nonce_binding_digest != expected_nonce_binding {
            return Err(RnsNativeRlweSourceStatementErrorV1::InvalidNonce);
        }
    }
    Ok(())
}

fn statement_anchor_digest_v1(
    qpcs_residual_digest: [u8; DIGEST_BYTES_V1],
    preflight_statement_digest: [u8; DIGEST_BYTES_V1],
    anchor_bytes: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRlweSourceStatementErrorV1> {
    if qpcs_residual_digest == [0; DIGEST_BYTES_V1]
        || preflight_statement_digest == [0; DIGEST_BYTES_V1]
        || anchor_bytes.is_empty()
        || anchor_bytes.len() > RNS_NATIVE_RLWE_SOURCE_RESIDUAL_MAX_BYTES_V1
    {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor);
    }
    let mut hash = Keccak256::new();
    hash.update(ANCHOR_DIGEST_DOMAIN_V1);
    hash.update(&[STATEMENT_VERSION_V1]);
    hash.update(&qpcs_residual_digest);
    hash.update(&preflight_statement_digest);
    hash.update(
        &u16::try_from(anchor_bytes.len())
            .map_err(|_| RnsNativeRlweSourceStatementErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(anchor_bytes);
    nonzero_digest_v1(hash.finalize())
        .map_err(|_| RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)
}

/// Move-only construction state after source semantics and the public statement preflight.
///
/// This owner proves neither RLWE equality nor qPCS/source linkage.  It cannot
/// authorize verification, readiness, or release and must be consumed by a
/// future concrete relation verifier.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "the following private RLWE relation milestone will consume this construction state exactly once"
)]
pub(super) struct RnsNativeRlweSourceStatementStageV1<'a, S: ZkAmsMkheRnsNativeSourceSnapshotV1> {
    qpcs: RnsNativeQpcsFriCompleteStageV1<'a>,
    snapshot: S,
    downstream: &'a [u8],
    epoch: u64,
    public_key_digest: [u8; DIGEST_BYTES_V1],
    public_bundle_digest: [u8; DIGEST_BYTES_V1],
    formula_digest: [u8; DIGEST_BYTES_V1],
    mapping_digest: [u8; DIGEST_BYTES_V1],
    aggregation_schedule_digest: [u8; DIGEST_BYTES_V1],
    preflight_statement_digest: [u8; DIGEST_BYTES_V1],
    statement_anchor_digest: [u8; DIGEST_BYTES_V1],
    challenges: [[AggregationChallengeV1; REPETITION_COUNT_V1]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
}

struct ValidatedPreflightPartsV1<'a, S: ZkAmsMkheRnsNativeSourceSnapshotV1> {
    snapshot: S,
    anchor: ResidualAnchorV1<'a>,
    derived: DerivedStatementV1,
    statement_anchor_digest: [u8; DIGEST_BYTES_V1],
}

#[allow(clippy::too_many_arguments)]
fn validate_preflight_parts_v1<'a, S>(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    public: RnsNativePublicArtifactViewV1<'_>,
    equation_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
    limb_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
    mut snapshot: S,
    qpcs: QpcsBindingsV1<'a>,
) -> Result<ValidatedPreflightPartsV1<'a, S>, RnsNativeRlweSourceStatementErrorV1>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let derived = derive_statement_v1(
        transcript,
        layout,
        receipt,
        public,
        equation_commitment_digests,
        limb_commitment_digests,
        qpcs,
    )?;
    let anchor = ResidualAnchorV1::from_canonical_bytes_exact_v1(qpcs.residual)?;
    if anchor.epoch != public.epoch
        || anchor.core_digests
            != expected_anchor_core_v1(
                transcript,
                layout,
                receipt,
                qpcs,
                derived,
                anchor.downstream,
            )?
    {
        return Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor);
    }
    validate_source_snapshot_v1(
        &mut snapshot,
        layout,
        receipt,
        public,
        derived.public_key_digest,
    )?;
    let statement_anchor_digest = statement_anchor_digest_v1(
        qpcs.residual_digest,
        derived.preflight_statement_digest,
        qpcs.residual,
    )?;
    Ok(ValidatedPreflightPartsV1 {
        snapshot,
        anchor,
        derived,
        statement_anchor_digest,
    })
}

#[allow(
    dead_code,
    reason = "retained construction bindings are consumed by the next private RLWE relation stage"
)]
impl<'a, S: ZkAmsMkheRnsNativeSourceSnapshotV1> RnsNativeRlweSourceStatementStageV1<'a, S> {
    pub(super) fn take_qpcs_relation_schedule_v1(
        &mut self,
    ) -> Result<RnsNativeQpcsRelationScheduleV1, RnsNativeQpcsFriCompleteErrorV1> {
        self.qpcs.take_relation_schedule_v1()
    }

    pub(super) const fn downstream(&self) -> &'a [u8] {
        self.downstream
    }

    pub(super) const fn epoch(&self) -> u64 {
        self.epoch
    }

    pub(super) const fn public_key_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.public_key_digest
    }

    pub(super) const fn public_bundle_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.public_bundle_digest
    }

    pub(super) const fn formula_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.formula_digest
    }

    pub(super) const fn mapping_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.mapping_digest
    }

    pub(super) const fn aggregation_schedule_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.aggregation_schedule_digest
    }

    pub(super) const fn preflight_statement_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.preflight_statement_digest
    }

    pub(super) const fn statement_anchor_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.statement_anchor_digest
    }

    pub(super) fn aggregation_challenge(
        &self,
        limb: usize,
        repetition: usize,
    ) -> Option<(u64, u64)> {
        self.challenges
            .get(limb)
            .and_then(|values| values.get(repetition))
            .map(|challenge| (challenge.gamma, challenge.beta))
    }

    pub(super) const fn qpcs(&self) -> &RnsNativeQpcsFriCompleteStageV1<'a> {
        &self.qpcs
    }

    pub(super) const fn snapshot(&self) -> &S {
        &self.snapshot
    }

    pub(super) const fn snapshot_mut(&mut self) -> &mut S {
        &mut self.snapshot
    }
}

/// Consume the complete qPCS sequencing token and source snapshot into a
/// non-authorizing RLWE/source-statement prerequisite.
#[allow(clippy::too_many_arguments)]
pub(super) fn preflight_rns_native_rlwe_source_statement_v1<'a, S>(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    public: RnsNativePublicArtifactViewV1<'_>,
    equation_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
    limb_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
    snapshot: S,
    qpcs: RnsNativeQpcsFriCompleteStageV1<'a>,
) -> Result<RnsNativeRlweSourceStatementStageV1<'a, S>, RnsNativeRlweSourceStatementErrorV1>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let qpcs_bindings = QpcsBindingsV1::from_stage_v1(&qpcs);
    let parts = validate_preflight_parts_v1(
        transcript,
        layout,
        receipt,
        public,
        equation_commitment_digests,
        limb_commitment_digests,
        snapshot,
        qpcs_bindings,
    )?;
    Ok(RnsNativeRlweSourceStatementStageV1 {
        qpcs,
        snapshot: parts.snapshot,
        downstream: parts.anchor.downstream,
        epoch: parts.anchor.epoch,
        public_key_digest: parts.derived.public_key_digest,
        public_bundle_digest: parts.derived.public_bundle_digest,
        formula_digest: parts.derived.formula_digest,
        mapping_digest: parts.derived.mapping_digest,
        aggregation_schedule_digest: parts.derived.aggregation_schedule_digest,
        preflight_statement_digest: parts.derived.preflight_statement_digest,
        statement_anchor_digest: parts.statement_anchor_digest,
        challenges: parts.derived.challenges,
    })
}

#[cfg(test)]
#[path = "rns_native_rlwe_source_statement_tests.rs"]
mod tests;
