//! Private transcript and relation kernel for the ZK-AMS RNS-Link v1 design.
//!
//! RNS-Link is the first-release replacement for the rejected linear-response
//! hidden-mask transport.  It will bind the six Phase-II/III witness families
//! to the same T256/Hyrax openings and to exact RNS-BGV encryption equations
//! through committed bit, packing, carry, and negacyclic-quotient tables.
//!
//! This checkpoint intentionally contains no public prover, verifier, or
//! accepting release gate.  It freezes the private proof kernel and its strict
//! canonical codec before the larger RNS relation is implemented:
//!
//! * every statement/context object and every family commitment is sealed in
//!   canonical order before Fiat--Shamir evaluation points can be derived;
//! * a transcript-bound cubic bitness sumcheck and logarithmic T256 IPA share
//!   one commitment and reject proof splicing;
//! * a sibling-private native receipt can bind one canonical packed chunk to
//!   the exact 38-limb plaintext lift and both state-owned RLWE equations; it
//!   deliberately does not authenticate any whole-proof wire response;
//! * the exact integer and arbitrary-point form of
//!   `A*R + p*E + M - C = (X^N + 1)*H (mod q)` is exercised by a tiny oracle,
//!   including the radix carry equation that prevents modular wraparound from
//!   masquerading as an integer opening.
//!
//! The operational boundary remains private and fail-closed until the complete
//! packing/carry/quotient relation, measured resource certificate, and
//! release-parameter KAT all exist.

#![allow(dead_code)]

use core::fmt;

use crate::vega::{
    VegaT256PointV1, VegaT256ScalarV1 as Scalar, VegaTranscriptV1,
    algebra::{decompress_univariate, eq_evals, eq_evaluate, evaluate_univariate},
    bulletproof_t256::{
        ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1, ZeroizingT256ScalarCopyV1,
        ZeroizingT256ScalarVecV1,
    },
    derive_t256_generators_v1,
    masked_relaxed::MASKED_RELAXED_COMMITMENT_COLUMNS_V1,
    sponge::{keccak256, shake256},
    sumcheck::{CompressedUnivariate, SumcheckProof},
};

use super::{
    ZkAmsMkheErrorV1,
    collective::{
        ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheCollectiveEncryptionOpeningV1,
        ZkAmsMkheCollectivePublicKeyV1,
    },
    manifest::{ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1, release_profile_v1},
    packing::{
        ZkAmsT256PackedPlaintextV1, ZkAmsT256PackingLayoutV1,
        decode_zk_ams_t256_packed_plaintext_v1, packed_plaintext_rns_binding_digest_v1,
    },
    phase23_encrypted::{
        ZK_AMS_PHASE23_RELEASE_ERROR_COMMITMENT_ROWS_V1,
        ZK_AMS_PHASE23_RELEASE_WITNESS_COMMITMENT_ROWS_V1,
        zk_ams_phase23_release_map_set_digest_v1,
    },
    wire::ZK_AMS_MKHE_MAX_PROOF_BYTES_V1,
};

const RNS_LINK_VERSION_V1: u8 = 1;
const RNS_LINK_EVALUATIONS_PER_LIMB_V1: usize = 5;
pub(super) const ZK_AMS_PHASE23_RNS_LINK_RELEASE_RNS_LIMB_COUNT_V1: usize = 38;
const RNS_LINK_REJECTION_ATTEMPTS_V1: usize = 128;
const RNS_LINK_FAMILY_COUNT_V1: usize = 6;
const RNS_LINK_MAX_CHUNKS_PER_FAMILY_V1: usize = 16;
const RNS_LINK_MAX_LOGICAL_VALUES_V1: usize =
    ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1 * RNS_LINK_MAX_CHUNKS_PER_FAMILY_V1;
const RNS_LINK_X_LOGICAL_VALUES_V1: usize =
    ZK_AMS_PHASE23_RELEASE_WITNESS_COMMITMENT_ROWS_V1 * MASKED_RELAXED_COMMITMENT_COLUMNS_V1;
const RNS_LINK_U_LOGICAL_VALUES_V1: usize = 1;
const RNS_LINK_E_LOGICAL_VALUES_V1: usize =
    ZK_AMS_PHASE23_RELEASE_ERROR_COMMITMENT_ROWS_V1 * MASKED_RELAXED_COMMITMENT_COLUMNS_V1;
const RNS_LINK_RE_LOGICAL_VALUES_V1: usize = ZK_AMS_PHASE23_RELEASE_ERROR_COMMITMENT_ROWS_V1;
const RNS_LINK_W_LOGICAL_VALUES_V1: usize = RNS_LINK_X_LOGICAL_VALUES_V1;
const RNS_LINK_RW_LOGICAL_VALUES_V1: usize = ZK_AMS_PHASE23_RELEASE_WITNESS_COMMITMENT_ROWS_V1;
const RNS_LINK_RELEASE_COMMITMENTS_V1: usize = 8 + 1 + 16 + 1 + 8 + 1;
const RNS_LINK_IPA_MAX_VECTOR_LEN_V1: usize = 2_048;
const RNS_LINK_IPA_MAX_ROUNDS_V1: usize = 11;
const RNS_LINK_IPA_CHALLENGE_RETRIES_V1: usize = 128;
const RNS_LINK_BITNESS_MAX_VALUES_V1: usize = 1_024;
const RNS_LINK_BITNESS_MAX_SUMCHECK_ROUNDS_V1: usize = 10;
const RNS_LINK_BITNESS_SUMCHECK_DEGREE_V1: usize = 3;
const RNS_LINK_SCALAR_WIRE_BYTES_V1: usize = 32;
const RNS_LINK_POINT_WIRE_BYTES_V1: usize = 33;
const RNS_LINK_BITNESS_CODEC_MAGIC_V1: [u8; 8] = *b"ZKRNBIT1";
const RNS_LINK_BITNESS_CODEC_HEADER_BYTES_V1: usize = 112;
const RNS_LINK_BITNESS_CODEC_MAX_BODY_BYTES_V1: usize = RNS_LINK_BITNESS_MAX_SUMCHECK_ROUNDS_V1
    * RNS_LINK_BITNESS_SUMCHECK_DEGREE_V1
    * RNS_LINK_SCALAR_WIRE_BYTES_V1
    + RNS_LINK_SCALAR_WIRE_BYTES_V1
    + 2 * RNS_LINK_IPA_MAX_ROUNDS_V1 * RNS_LINK_POINT_WIRE_BYTES_V1
    + RNS_LINK_SCALAR_WIRE_BYTES_V1;
const RNS_LINK_BITNESS_CODEC_MAX_BYTES_V1: usize =
    RNS_LINK_BITNESS_CODEC_HEADER_BYTES_V1 + RNS_LINK_BITNESS_CODEC_MAX_BODY_BYTES_V1;

const BITNESS_CODEC_MANIFEST_OFFSET_V1: usize = 12;
const BITNESS_CODEC_VALUE_COUNT_OFFSET_V1: usize = 44;
const BITNESS_CODEC_SUMCHECK_ROUNDS_OFFSET_V1: usize = 46;
const BITNESS_CODEC_SUMCHECK_DEGREE_OFFSET_V1: usize = 47;
const BITNESS_CODEC_SUMCHECK_COEFFICIENT_COUNT_OFFSET_V1: usize = 48;
const BITNESS_CODEC_IPA_ROUNDS_OFFSET_V1: usize = 50;
const BITNESS_CODEC_IPA_LEFT_COUNT_OFFSET_V1: usize = 51;
const BITNESS_CODEC_IPA_RIGHT_COUNT_OFFSET_V1: usize = 52;
const BITNESS_CODEC_POINT_BYTES_OFFSET_V1: usize = 53;
const BITNESS_CODEC_SCALAR_BYTES_OFFSET_V1: usize = 54;
const BITNESS_CODEC_RESERVED_OFFSET_V1: usize = 55;
const BITNESS_CODEC_SUMCHECK_BYTES_OFFSET_V1: usize = 56;
const BITNESS_CODEC_EVALUATION_BYTES_OFFSET_V1: usize = 64;
const BITNESS_CODEC_IPA_LEFT_BYTES_OFFSET_V1: usize = 72;
const BITNESS_CODEC_IPA_RIGHT_BYTES_OFFSET_V1: usize = 80;
const BITNESS_CODEC_FINAL_WITNESS_BYTES_OFFSET_V1: usize = 88;
const BITNESS_CODEC_BODY_BYTES_OFFSET_V1: usize = 96;
const BITNESS_CODEC_TOTAL_BYTES_OFFSET_V1: usize = 104;

const CONTEXT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.rns-link.context";
const ALGORITHM_MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.immutable-algorithm-manifest";
const COMMITMENT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.rns-link.commitment";
const COMMITMENT_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.ordered-commitment-root";
const PRECHALLENGE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.rns-link.prechallenge";
const EVALUATION_POINT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.rns-link.evaluation-point";
const CHALLENGE_SET_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.rns-link.challenge-set";
const IPA_TRANSCRIPT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.rns-link.ipa";
const BITNESS_SUMCHECK_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.rns-link.bitness-sumcheck";
const NATIVE_BGV_OPENING_RECEIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.native-bgv-opening-receipt";
const IPA_GENERATOR_LABEL_V1: &[u8] = b"iroha.zk-ams.v1.phase23.rns-link.ipa-generators";
const BITNESS_ALGORITHM_LABEL_V1: &[u8] = b"rns-link-algorithm";
const BITNESS_CONTEXT_LABEL_V1: &[u8] = b"rns-link-context";
const BITNESS_IPA_KEY_LABEL_V1: &[u8] = b"rns-link-ipa-key";
const BITNESS_VALUE_COUNT_LABEL_V1: &[u8] = b"rns-link-value-count";
const BITNESS_COMMITMENT_LABEL_V1: &[u8] = b"rns-link-bit-commitment";
const BITNESS_TAU_CHALLENGE_LABEL_V1: &[u8] = b"rns-link-bitness-tau";
const BITNESS_SUMCHECK_POLYNOMIAL_LABEL_V1: &[u8] = b"p";
const BITNESS_SUMCHECK_CHALLENGE_LABEL_V1: &[u8] = b"c";
const IPA_EVALUATION_LABEL_V1: &[u8] = b"rns-link-ipa-evaluation";
const IPA_LEFT_LABEL_V1: &[u8] = b"rns-link-ipa-left";
const IPA_RIGHT_LABEL_V1: &[u8] = b"rns-link-ipa-right";
const IPA_CHALLENGE_LABEL_V1: &[u8] = b"rns-link-ipa-challenge";

const RNS_LINK_MANIFEST_DOMAINS_V1: [&[u8]; 8] = [
    CONTEXT_DOMAIN_V1,
    COMMITMENT_DOMAIN_V1,
    COMMITMENT_ROOT_DOMAIN_V1,
    PRECHALLENGE_DOMAIN_V1,
    EVALUATION_POINT_DOMAIN_V1,
    CHALLENGE_SET_DOMAIN_V1,
    IPA_TRANSCRIPT_DOMAIN_V1,
    BITNESS_SUMCHECK_DOMAIN_V1,
];

const RNS_LINK_MANIFEST_TRANSCRIPT_LABELS_V1: [&[u8]; 13] = [
    IPA_GENERATOR_LABEL_V1,
    BITNESS_ALGORITHM_LABEL_V1,
    BITNESS_CONTEXT_LABEL_V1,
    BITNESS_IPA_KEY_LABEL_V1,
    BITNESS_VALUE_COUNT_LABEL_V1,
    BITNESS_COMMITMENT_LABEL_V1,
    BITNESS_TAU_CHALLENGE_LABEL_V1,
    BITNESS_SUMCHECK_POLYNOMIAL_LABEL_V1,
    BITNESS_SUMCHECK_CHALLENGE_LABEL_V1,
    IPA_EVALUATION_LABEL_V1,
    IPA_LEFT_LABEL_V1,
    IPA_RIGHT_LABEL_V1,
    IPA_CHALLENGE_LABEL_V1,
];

const BITNESS_RELATION_DESCRIPTOR_V1: &[u8] =
    b"sum_x:eq(tau,x)*b(x)*(b(x)-1)=0:cubic:compressed-constant-quadratic-cubic";
const IPA_RELATION_DESCRIPTOR_V1: &[u8] =
    b"P=<a,G>+<a,b>*U:fold-a=x*aL+x^-1*aR:fold-b=x^-1*bL+x*bR:fold-G=x^-1*GL+x*GR";
const BITNESS_CODEC_SCHEMA_DESCRIPTOR_V1: &[u8] = b"header-be:magic[0..8],version8,flags9,header_len10..12,manifest12..44,value_count44..46,sumcheck_rounds46,degree47,coeff_count48..50,ipa_rounds50,left_count51,right_count52,point_bytes53,scalar_bytes54,reserved55,sumcheck_len56..64,evaluation_len64..72,left_len72..80,right_len80..88,final_len88..96,body_len96..104,total_len104..112;body=sumcheck_be||evaluation_be||left_points||right_points||final_be";
const RNS_LINK_MANIFEST_FORMAT_DESCRIPTORS_V1: [&[u8]; 3] = [
    BITNESS_RELATION_DESCRIPTOR_V1,
    IPA_RELATION_DESCRIPTOR_V1,
    BITNESS_CODEC_SCHEMA_DESCRIPTOR_V1,
];

const _: () = {
    assert!(RNS_LINK_EVALUATIONS_PER_LIMB_V1 == 5);
    assert!(RNS_LINK_REJECTION_ATTEMPTS_V1 == 128);
    assert!(RNS_LINK_MAX_LOGICAL_VALUES_V1 == 1_048_576);
    assert!(RNS_LINK_X_LOGICAL_VALUES_V1 == 524_288);
    assert!(RNS_LINK_E_LOGICAL_VALUES_V1 == 1_048_576);
    assert!(RNS_LINK_RE_LOGICAL_VALUES_V1 == 1_024);
    assert!(RNS_LINK_RW_LOGICAL_VALUES_V1 == 512);
    assert!(RNS_LINK_RELEASE_COMMITMENTS_V1 == 35);
    assert!(RNS_LINK_IPA_MAX_VECTOR_LEN_V1 == 1 << RNS_LINK_IPA_MAX_ROUNDS_V1);
    assert!(RNS_LINK_IPA_CHALLENGE_RETRIES_V1 == 128);
    assert!(RNS_LINK_BITNESS_MAX_VALUES_V1 == 1 << RNS_LINK_BITNESS_MAX_SUMCHECK_ROUNDS_V1);
    assert!(RNS_LINK_BITNESS_SUMCHECK_DEGREE_V1 == 3);
    assert!(RNS_LINK_SCALAR_WIRE_BYTES_V1 == 32);
    assert!(RNS_LINK_POINT_WIRE_BYTES_V1 == 33);
    assert!(RNS_LINK_BITNESS_CODEC_HEADER_BYTES_V1 == 112);
    assert!(RNS_LINK_BITNESS_CODEC_MAX_BODY_BYTES_V1 == 1_750);
    assert!(RNS_LINK_BITNESS_CODEC_MAX_BYTES_V1 == 1_862);
    assert!(RNS_LINK_BITNESS_CODEC_MAX_BYTES_V1 < ZK_AMS_MKHE_MAX_PROOF_BYTES_V1);
};

#[cfg(test)]
std::thread_local! {
    static RNS_LINK_IPA_KEY_DERIVATIONS_V1: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static RNS_LINK_CODEC_BODY_ALLOCATIONS_V1: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn is_nonzero_digest(digest: [u8; 32]) -> bool {
    digest != [0; 32]
}

/// The six logical witness families in their only accepted transcript order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(super) enum ZkAmsPhase23RnsLinkFamilyV1 {
    X = 1,
    U = 2,
    E = 3,
    RE = 4,
    W = 5,
    RW = 6,
}

const RNS_LINK_FAMILY_ORDER_V1: [ZkAmsPhase23RnsLinkFamilyV1; RNS_LINK_FAMILY_COUNT_V1] = [
    ZkAmsPhase23RnsLinkFamilyV1::X,
    ZkAmsPhase23RnsLinkFamilyV1::U,
    ZkAmsPhase23RnsLinkFamilyV1::E,
    ZkAmsPhase23RnsLinkFamilyV1::RE,
    ZkAmsPhase23RnsLinkFamilyV1::W,
    ZkAmsPhase23RnsLinkFamilyV1::RW,
];

fn expected_logical_values_v1(family: ZkAmsPhase23RnsLinkFamilyV1) -> usize {
    match family {
        ZkAmsPhase23RnsLinkFamilyV1::X => RNS_LINK_X_LOGICAL_VALUES_V1,
        ZkAmsPhase23RnsLinkFamilyV1::U => RNS_LINK_U_LOGICAL_VALUES_V1,
        ZkAmsPhase23RnsLinkFamilyV1::E => RNS_LINK_E_LOGICAL_VALUES_V1,
        ZkAmsPhase23RnsLinkFamilyV1::RE => RNS_LINK_RE_LOGICAL_VALUES_V1,
        ZkAmsPhase23RnsLinkFamilyV1::W => RNS_LINK_W_LOGICAL_VALUES_V1,
        ZkAmsPhase23RnsLinkFamilyV1::RW => RNS_LINK_RW_LOGICAL_VALUES_V1,
    }
}

#[derive(Clone)]
struct RnsLinkImmutableAlgorithmManifestInputsV1 {
    version: u8,
    profile_digest: [u8; 32],
    map_set_digest: [u8; 32],
    generator_basis_digest: [u8; 32],
    dimensions: [u64; 21],
    family_shapes: [(u8, u64); RNS_LINK_FAMILY_COUNT_V1],
    domains: [&'static [u8]; RNS_LINK_MANIFEST_DOMAINS_V1.len()],
    transcript_labels: [&'static [u8]; RNS_LINK_MANIFEST_TRANSCRIPT_LABELS_V1.len()],
    format_descriptors: [&'static [u8]; RNS_LINK_MANIFEST_FORMAT_DESCRIPTORS_V1.len()],
    codec_magic: [u8; RNS_LINK_BITNESS_CODEC_MAGIC_V1.len()],
}

fn usize_as_manifest_u64_v1(value: usize) -> Result<u64, ZkAmsMkheErrorV1> {
    u64::try_from(value).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn canonical_algorithm_manifest_inputs_v1()
-> Result<RnsLinkImmutableAlgorithmManifestInputsV1, ZkAmsMkheErrorV1> {
    let dimensions = [
        usize_as_manifest_u64_v1(ZK_AMS_MKHE_MAX_PROOF_BYTES_V1)?,
        usize_as_manifest_u64_v1(ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1)?,
        usize_as_manifest_u64_v1(MASKED_RELAXED_COMMITMENT_COLUMNS_V1)?,
        usize_as_manifest_u64_v1(RNS_LINK_EVALUATIONS_PER_LIMB_V1)?,
        usize_as_manifest_u64_v1(RNS_LINK_REJECTION_ATTEMPTS_V1)?,
        usize_as_manifest_u64_v1(RNS_LINK_FAMILY_COUNT_V1)?,
        usize_as_manifest_u64_v1(RNS_LINK_MAX_CHUNKS_PER_FAMILY_V1)?,
        usize_as_manifest_u64_v1(RNS_LINK_MAX_LOGICAL_VALUES_V1)?,
        usize_as_manifest_u64_v1(RNS_LINK_RELEASE_COMMITMENTS_V1)?,
        usize_as_manifest_u64_v1(RNS_LINK_IPA_MAX_VECTOR_LEN_V1)?,
        usize_as_manifest_u64_v1(RNS_LINK_IPA_MAX_ROUNDS_V1)?,
        usize_as_manifest_u64_v1(RNS_LINK_IPA_CHALLENGE_RETRIES_V1)?,
        usize_as_manifest_u64_v1(RNS_LINK_BITNESS_MAX_VALUES_V1)?,
        usize_as_manifest_u64_v1(RNS_LINK_BITNESS_MAX_SUMCHECK_ROUNDS_V1)?,
        usize_as_manifest_u64_v1(RNS_LINK_BITNESS_SUMCHECK_DEGREE_V1)?,
        usize_as_manifest_u64_v1(RNS_LINK_SCALAR_WIRE_BYTES_V1)?,
        usize_as_manifest_u64_v1(RNS_LINK_POINT_WIRE_BYTES_V1)?,
        usize_as_manifest_u64_v1(RNS_LINK_BITNESS_CODEC_HEADER_BYTES_V1)?,
        usize_as_manifest_u64_v1(RNS_LINK_BITNESS_CODEC_MAX_BODY_BYTES_V1)?,
        usize_as_manifest_u64_v1(RNS_LINK_BITNESS_CODEC_MAX_BYTES_V1)?,
        1, // Exactly one Pedersen blinding coordinate, with public weight zero.
    ];
    let mut family_shapes = [(0_u8, 0_u64); RNS_LINK_FAMILY_COUNT_V1];
    for (index, family) in RNS_LINK_FAMILY_ORDER_V1.iter().copied().enumerate() {
        family_shapes[index] = (
            family as u8,
            usize_as_manifest_u64_v1(expected_logical_values_v1(family))?,
        );
    }
    Ok(RnsLinkImmutableAlgorithmManifestInputsV1 {
        version: RNS_LINK_VERSION_V1,
        profile_digest: release_profile_v1().digest()?,
        map_set_digest: zk_ams_phase23_release_map_set_digest_v1()?,
        generator_basis_digest: ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
        dimensions,
        family_shapes,
        domains: RNS_LINK_MANIFEST_DOMAINS_V1,
        transcript_labels: RNS_LINK_MANIFEST_TRANSCRIPT_LABELS_V1,
        format_descriptors: RNS_LINK_MANIFEST_FORMAT_DESCRIPTORS_V1,
        codec_magic: RNS_LINK_BITNESS_CODEC_MAGIC_V1,
    })
}

fn append_manifest_byte_strings_v1(
    frame: &mut Vec<u8>,
    values: &[&[u8]],
) -> Result<(), ZkAmsMkheErrorV1> {
    frame.extend_from_slice(
        &u16::try_from(values.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    for value in values {
        frame.extend_from_slice(
            &u16::try_from(value.len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        frame.extend_from_slice(value);
    }
    Ok(())
}

fn immutable_algorithm_manifest_digest_from_inputs_v1(
    inputs: &RnsLinkImmutableAlgorithmManifestInputsV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut frame = Vec::with_capacity(ALGORITHM_MANIFEST_DOMAIN_V1.len() + 1_024);
    frame.extend_from_slice(ALGORITHM_MANIFEST_DOMAIN_V1);
    frame.push(inputs.version);
    frame.extend_from_slice(&inputs.profile_digest);
    frame.extend_from_slice(&inputs.map_set_digest);
    frame.extend_from_slice(&inputs.generator_basis_digest);
    frame.extend_from_slice(
        &u16::try_from(inputs.dimensions.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    for dimension in inputs.dimensions {
        frame.extend_from_slice(&dimension.to_be_bytes());
    }
    frame.extend_from_slice(
        &u16::try_from(inputs.family_shapes.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    for (family, logical_values) in inputs.family_shapes {
        frame.push(family);
        frame.extend_from_slice(&logical_values.to_be_bytes());
    }
    append_manifest_byte_strings_v1(&mut frame, &inputs.domains)?;
    append_manifest_byte_strings_v1(&mut frame, &inputs.transcript_labels)?;
    append_manifest_byte_strings_v1(&mut frame, &inputs.format_descriptors)?;
    frame.extend_from_slice(
        &u16::try_from(inputs.codec_magic.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    frame.extend_from_slice(&inputs.codec_magic);
    Ok(keccak256(&frame))
}

/// Digest only immutable proof-algorithm inputs. Mutable readiness flags,
/// measured evidence, and release-KAT pins are deliberately absent: including
/// any of them would make installing a KAT change the proof it is meant to pin.
fn immutable_algorithm_manifest_digest_v1() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    immutable_algorithm_manifest_digest_from_inputs_v1(&canonical_algorithm_manifest_inputs_v1()?)
}

/// Every immutable context axis bound before any RNS-Link commitment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsPhase23RnsLinkContextV1 {
    profile_digest: [u8; 32],
    algorithm_manifest_digest: [u8; 32],
    network_context_digest: [u8; 32],
    statement_context_digest: [u8; 32],
    transcript_digest: [u8; 32],
    batch_digest: [u8; 32],
    roster_digest: [u8; 32],
    direct_key_admission_digest: [u8; 32],
    canonical_map_set_digest: [u8; 32],
}

impl ZkAmsPhase23RnsLinkContextV1 {
    /// Construct the release-profile context. The immutable profile and
    /// algorithm manifest are derived internally. Readiness and release-KAT
    /// evidence are intentionally enforced outside proof verification.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        network_context_digest: [u8; 32],
        statement_context_digest: [u8; 32],
        transcript_digest: [u8; 32],
        batch_digest: [u8; 32],
        roster_digest: [u8; 32],
        direct_key_admission_digest: [u8; 32],
        canonical_map_set_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let supplied = [
            network_context_digest,
            statement_context_digest,
            transcript_digest,
            batch_digest,
            roster_digest,
            direct_key_admission_digest,
            canonical_map_set_digest,
        ];
        if supplied
            .iter()
            .copied()
            .any(|digest| !is_nonzero_digest(digest))
            || canonical_map_set_digest != zk_ams_phase23_release_map_set_digest_v1()?
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(Self {
            profile_digest: release_profile_v1().digest()?,
            algorithm_manifest_digest: immutable_algorithm_manifest_digest_v1()?,
            network_context_digest,
            statement_context_digest,
            transcript_digest,
            batch_digest,
            roster_digest,
            direct_key_admission_digest,
            canonical_map_set_digest,
        })
    }

    fn digest(self) -> [u8; 32] {
        let mut frame = Vec::with_capacity(CONTEXT_DOMAIN_V1.len() + 2 + 9 * 32);
        frame.extend_from_slice(CONTEXT_DOMAIN_V1);
        frame.push(RNS_LINK_VERSION_V1);
        frame.extend_from_slice(&self.profile_digest);
        frame.extend_from_slice(&self.algorithm_manifest_digest);
        frame.extend_from_slice(&self.network_context_digest);
        frame.extend_from_slice(&self.statement_context_digest);
        frame.extend_from_slice(&self.transcript_digest);
        frame.extend_from_slice(&self.batch_digest);
        frame.extend_from_slice(&self.roster_digest);
        frame.extend_from_slice(&self.direct_key_admission_digest);
        frame.extend_from_slice(&self.canonical_map_set_digest);
        keccak256(&frame)
    }
}

/// Roots of all tables that must exist before Fiat--Shamir sampling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsPhase23RnsLinkCommitmentDigestsV1 {
    layout_digest: [u8; 32],
    ciphertext_digest: [u8; 32],
    bit_planes_digest: [u8; 32],
    small_openings_digest: [u8; 32],
    packing_trace_digest: [u8; 32],
    radix_carry_digest: [u8; 32],
    negacyclic_quotient_digest: [u8; 32],
    padding_digest: [u8; 32],
}

impl ZkAmsPhase23RnsLinkCommitmentDigestsV1 {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        layout_digest: [u8; 32],
        ciphertext_digest: [u8; 32],
        bit_planes_digest: [u8; 32],
        small_openings_digest: [u8; 32],
        packing_trace_digest: [u8; 32],
        radix_carry_digest: [u8; 32],
        negacyclic_quotient_digest: [u8; 32],
        padding_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let value = Self {
            layout_digest,
            ciphertext_digest,
            bit_planes_digest,
            small_openings_digest,
            packing_trace_digest,
            radix_carry_digest,
            negacyclic_quotient_digest,
            padding_digest,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(self) -> Result<(), ZkAmsMkheErrorV1> {
        let digests = [
            self.layout_digest,
            self.ciphertext_digest,
            self.bit_planes_digest,
            self.small_openings_digest,
            self.packing_trace_digest,
            self.radix_carry_digest,
            self.negacyclic_quotient_digest,
            self.padding_digest,
        ];
        if digests
            .iter()
            .copied()
            .any(|digest| !is_nonzero_digest(digest))
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }
}

/// One present ciphertext chunk and all tables cross-bound to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsPhase23RnsLinkChunkCommitmentV1 {
    family: ZkAmsPhase23RnsLinkFamilyV1,
    chunk_index: u16,
    chunk_count: u16,
    logical_value_count: u32,
    used_slots: u32,
    /// A one bit denotes an absent position in the fixed 16-chunk family
    /// namespace. This makes omitted trailing chunks explicit transcript data.
    absent_chunk_bitmap: u16,
    hyrax_commitment: [u8; 33],
    digests: ZkAmsPhase23RnsLinkCommitmentDigestsV1,
}

impl ZkAmsPhase23RnsLinkChunkCommitmentV1 {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        family: ZkAmsPhase23RnsLinkFamilyV1,
        chunk_index: u16,
        chunk_count: u16,
        logical_value_count: u32,
        used_slots: u32,
        absent_chunk_bitmap: u16,
        hyrax_commitment: [u8; 33],
        digests: ZkAmsPhase23RnsLinkCommitmentDigestsV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let commitment = Self {
            family,
            chunk_index,
            chunk_count,
            logical_value_count,
            used_slots,
            absent_chunk_bitmap,
            hyrax_commitment,
            digests,
        };
        commitment.validate()?;
        Ok(commitment)
    }

    fn validate(self) -> Result<(), ZkAmsMkheErrorV1> {
        let chunk_count = usize::from(self.chunk_count);
        let chunk_index = usize::from(self.chunk_index);
        let logical_value_count = usize::try_from(self.logical_value_count)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let expected_chunk_count = logical_value_count
            .checked_add(ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1 - 1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            / ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1;
        if logical_value_count == 0
            || logical_value_count > RNS_LINK_MAX_LOGICAL_VALUES_V1
            || logical_value_count != expected_logical_values_v1(self.family)
            || chunk_count == 0
            || chunk_count > RNS_LINK_MAX_CHUNKS_PER_FAMILY_V1
            || chunk_index >= chunk_count
            || chunk_count != expected_chunk_count
            || self.absent_chunk_bitmap != canonical_absent_chunk_bitmap_v1(self.chunk_count)?
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let consumed = chunk_index
            .checked_mul(ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let expected_used_slots = logical_value_count
            .checked_sub(consumed)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .min(ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1);
        if usize::try_from(self.used_slots)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            != expected_used_slots
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        VegaT256PointV1::from_non_identity_wire_bytes_exact(&self.hyrax_commitment)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        self.digests.validate()
    }

    fn digest(self) -> [u8; 32] {
        let mut frame = Vec::with_capacity(COMMITMENT_DOMAIN_V1.len() + 320);
        frame.extend_from_slice(COMMITMENT_DOMAIN_V1);
        frame.push(RNS_LINK_VERSION_V1);
        frame.push(self.family as u8);
        frame.extend_from_slice(&self.chunk_index.to_be_bytes());
        frame.extend_from_slice(&self.chunk_count.to_be_bytes());
        frame.extend_from_slice(&self.logical_value_count.to_be_bytes());
        frame.extend_from_slice(&self.used_slots.to_be_bytes());
        frame.extend_from_slice(&self.absent_chunk_bitmap.to_be_bytes());
        frame.extend_from_slice(&self.hyrax_commitment);
        frame.extend_from_slice(&self.digests.layout_digest);
        frame.extend_from_slice(&self.digests.ciphertext_digest);
        frame.extend_from_slice(&self.digests.bit_planes_digest);
        frame.extend_from_slice(&self.digests.small_openings_digest);
        frame.extend_from_slice(&self.digests.packing_trace_digest);
        frame.extend_from_slice(&self.digests.radix_carry_digest);
        frame.extend_from_slice(&self.digests.negacyclic_quotient_digest);
        frame.extend_from_slice(&self.digests.padding_digest);
        keccak256(&frame)
    }
}

fn canonical_absent_chunk_bitmap_v1(chunk_count: u16) -> Result<u16, ZkAmsMkheErrorV1> {
    let count = usize::from(chunk_count);
    if count == 0 || count > RNS_LINK_MAX_CHUNKS_PER_FAMILY_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let present = if count == u16::BITS as usize {
        u16::MAX
    } else {
        (1_u16 << count) - 1
    };
    Ok(!present)
}

/// A sealed, canonical commitment set. Challenge derivation accepts this type
/// rather than raw statement fields, making commit-before-challenge structural.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsPhase23RnsLinkPrechallengeV1 {
    context_digest: [u8; 32],
    ordered_commitment_root: [u8; 32],
    commitment_count: u16,
    transcript_digest: [u8; 32],
}

impl ZkAmsPhase23RnsLinkPrechallengeV1 {
    pub(super) fn from_ordered_commitments(
        context: ZkAmsPhase23RnsLinkContextV1,
        commitments: &[ZkAmsPhase23RnsLinkChunkCommitmentV1],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if commitments.len() != RNS_LINK_RELEASE_COMMITMENTS_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }

        let mut cursor = 0_usize;
        for family in RNS_LINK_FAMILY_ORDER_V1 {
            let first = commitments
                .get(cursor)
                .copied()
                .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
            first.validate()?;
            if first.family != family || first.chunk_index != 0 {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
            let family_chunks = usize::from(first.chunk_count);
            let end = cursor
                .checked_add(family_chunks)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            let group = commitments
                .get(cursor..end)
                .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
            for (expected_index, commitment) in group.iter().copied().enumerate() {
                commitment.validate()?;
                if commitment.family != family
                    || usize::from(commitment.chunk_index) != expected_index
                    || commitment.chunk_count != first.chunk_count
                    || commitment.logical_value_count != first.logical_value_count
                    || commitment.absent_chunk_bitmap != first.absent_chunk_bitmap
                    || commitment.digests.layout_digest != first.digests.layout_digest
                {
                    return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
                }
            }
            cursor = end;
        }
        if cursor != commitments.len() {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }

        for (index, commitment) in commitments.iter().copied().enumerate() {
            let digest = commitment.digest();
            if commitments[..index]
                .iter()
                .copied()
                .any(|prior| prior.digest() == digest)
            {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
        }

        let context_digest = context.digest();
        let commitment_count = u16::try_from(commitments.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let mut root_frame =
            Vec::with_capacity(COMMITMENT_ROOT_DOMAIN_V1.len() + 40 + commitments.len() * 32);
        root_frame.extend_from_slice(COMMITMENT_ROOT_DOMAIN_V1);
        root_frame.push(RNS_LINK_VERSION_V1);
        root_frame.extend_from_slice(&context_digest);
        root_frame.extend_from_slice(&commitment_count.to_be_bytes());
        for commitment in commitments.iter().copied() {
            root_frame.extend_from_slice(&commitment.digest());
        }
        let ordered_commitment_root = keccak256(&root_frame);

        let mut transcript_frame = Vec::with_capacity(PRECHALLENGE_DOMAIN_V1.len() + 70);
        transcript_frame.extend_from_slice(PRECHALLENGE_DOMAIN_V1);
        transcript_frame.push(RNS_LINK_VERSION_V1);
        transcript_frame.extend_from_slice(&context_digest);
        transcript_frame.extend_from_slice(&ordered_commitment_root);
        transcript_frame.extend_from_slice(&commitment_count.to_be_bytes());
        let transcript_digest = keccak256(&transcript_frame);
        Ok(Self {
            context_digest,
            ordered_commitment_root,
            commitment_count,
            transcript_digest,
        })
    }
}

/// One canonical, nonzero evaluation point for one RNS prime and repetition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsPhase23RnsLinkEvaluationPointV1 {
    limb_index: u8,
    repetition: u8,
    modulus: u64,
    value: u64,
}

/// Complete challenge set derived only after sealing every commitment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsPhase23RnsLinkChallengeSetV1 {
    prechallenge_digest: [u8; 32],
    modulus_set_digest: [u8; 32],
    points: Vec<ZkAmsPhase23RnsLinkEvaluationPointV1>,
    digest: [u8; 32],
}

impl ZkAmsPhase23RnsLinkChallengeSetV1 {
    fn validate_for_release(
        &self,
        prechallenge: &ZkAmsPhase23RnsLinkPrechallengeV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let expected = derive_release_evaluation_points_v1(prechallenge)?;
        if self != &expected {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }
}

fn derive_release_evaluation_points_v1(
    prechallenge: &ZkAmsPhase23RnsLinkPrechallengeV1,
) -> Result<ZkAmsPhase23RnsLinkChallengeSetV1, ZkAmsMkheErrorV1> {
    derive_evaluation_points_for_moduli_v1(prechallenge, release_profile_v1().moduli)
}

/// Verifier-owned binding between the canonical whole-proof transport and the
/// real release relation inputs.
///
/// `statement_digest` is the digest of the complete challenge set derived from
/// the context and all ordered commitments. It is never accepted from the
/// proof producer. The wire decoder may use this value to reject a digest shell
/// or a structurally valid envelope carrying commitments from another proof.
/// This binding deliberately makes no claim that the relation responses verify.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsPhase23RnsLinkWholeProofBindingV1 {
    pub(super) profile_digest: [u8; 32],
    pub(super) algorithm_manifest_digest: [u8; 32],
    pub(super) context_digest: [u8; 32],
    pub(super) statement_digest: [u8; 32],
    pub(super) ordered_commitment_root: [u8; 32],
    pub(super) hyrax_commitments: [VegaT256PointV1; RNS_LINK_RELEASE_COMMITMENTS_V1],
}

impl ZkAmsPhase23RnsLinkWholeProofBindingV1 {
    /// Recompute every transport-binding field from verifier-owned native
    /// relation types. No digest supplied by a proof producer is an input.
    pub(super) fn derive(
        context: ZkAmsPhase23RnsLinkContextV1,
        commitments: &[ZkAmsPhase23RnsLinkChunkCommitmentV1],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if context.profile_digest != release_profile_v1().digest()?
            || context.algorithm_manifest_digest != immutable_algorithm_manifest_digest_v1()?
            || release_profile_v1().moduli.len()
                != ZK_AMS_PHASE23_RNS_LINK_RELEASE_RNS_LIMB_COUNT_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let prechallenge =
            ZkAmsPhase23RnsLinkPrechallengeV1::from_ordered_commitments(context, commitments)?;
        let challenge_set = derive_release_evaluation_points_v1(&prechallenge)?;
        challenge_set.validate_for_release(&prechallenge)?;
        if challenge_set.points.len()
            != release_profile_v1()
                .moduli
                .len()
                .checked_mul(RNS_LINK_EVALUATIONS_PER_LIMB_V1)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }

        let mut hyrax_commitments = Vec::new();
        hyrax_commitments
            .try_reserve_exact(RNS_LINK_RELEASE_COMMITMENTS_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for commitment in commitments {
            commitment.validate()?;
            hyrax_commitments.push(
                VegaT256PointV1::from_non_identity_wire_bytes_exact(&commitment.hyrax_commitment)
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
            );
        }

        Ok(Self {
            profile_digest: context.profile_digest,
            algorithm_manifest_digest: context.algorithm_manifest_digest,
            context_digest: context.digest(),
            statement_digest: challenge_set.digest,
            ordered_commitment_root: prechallenge.ordered_commitment_root,
            hyrax_commitments: hyrax_commitments
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
        })
    }
}

/// Opaque capability for one native, in-process packed BGV opening.
///
/// This is intentionally narrower than an RNS-Link proof receipt. It is minted
/// only while the state-owned encryption opening is available and after the
/// canonical T256 packing (including zero padding), its exact 38-limb RNS
/// image, and both native RLWE equations have been recomputed. It neither
/// authenticates the carry/quotient records in the whole-proof wire envelope
/// nor proves equality to a Hyrax commitment.
pub(super) struct VerifiedZkAmsPhase23NativeBgvOpeningV1 {
    key_digest: [u8; 32],
    layout_digest: [u8; 32],
    plaintext_digest: [u8; 32],
    ciphertext_digest: [u8; 32],
    rns_binding_digest: [u8; 32],
    digest: [u8; 32],
}

impl fmt::Debug for VerifiedZkAmsPhase23NativeBgvOpeningV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedZkAmsPhase23NativeBgvOpeningV1")
            .field("key_digest", &hex::encode(self.key_digest))
            .field("layout_digest", &hex::encode(self.layout_digest))
            .field("plaintext_digest", &hex::encode(self.plaintext_digest))
            .field("ciphertext_digest", &hex::encode(self.ciphertext_digest))
            .field("rns_binding_digest", &hex::encode(self.rns_binding_digest))
            .field("digest", &hex::encode(self.digest))
            .finish()
    }
}

impl VerifiedZkAmsPhase23NativeBgvOpeningV1 {
    /// Recheck that this opaque capability names the exact public artifacts a
    /// sibling consumer intends to use. The secret opening is not retained.
    pub(super) fn validate_for(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        layout: ZkAmsT256PackingLayoutV1,
        plaintext: &ZkAmsT256PackedPlaintextV1,
        ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if self.key_digest != key.digest()
            || self.layout_digest != layout.digest
            || self.plaintext_digest != plaintext.digest
            || self.ciphertext_digest != ciphertext.digest()
            || self.rns_binding_digest == [0; 32]
            || self.digest == [0; 32]
            || self.digest != native_bgv_opening_receipt_digest_v1(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }
}

/// Verify one real release-profile encryption opening and mint a capability
/// that contains no witness material.
pub(super) fn verify_zk_ams_phase23_native_bgv_opening_v1(
    key: &ZkAmsMkheCollectivePublicKeyV1,
    layout: ZkAmsT256PackingLayoutV1,
    plaintext: &ZkAmsT256PackedPlaintextV1,
    ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    opening: &ZkAmsMkheCollectiveEncryptionOpeningV1,
) -> Result<VerifiedZkAmsPhase23NativeBgvOpeningV1, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    if profile.moduli.len() != ZK_AMS_PHASE23_RNS_LINK_RELEASE_RNS_LIMB_COUNT_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }

    // Decoding independently rechecks the exact layout, canonical packed
    // digest, chunk metadata, and every unused slot before any receipt exists.
    let decoded = decode_zk_ams_t256_packed_plaintext_v1(layout, plaintext)?;
    let used_slots = usize::try_from(plaintext.used_slots)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if decoded.len() != ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1
        || used_slots > decoded.len()
        || decoded[used_slots..].iter().any(|value| *value != [0; 32])
    {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let rns_binding_digest = packed_plaintext_rns_binding_digest_v1(layout, plaintext)?;
    if rns_binding_digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }

    // This adapter validates the complete public context, the state-owned
    // canonical/RNS plaintext identity, witness bounds, and both release-RNS
    // RLWE equations before invoking the closure.
    opening.with_validated_proof_witness_v1(
        key,
        layout,
        plaintext,
        ciphertext,
        |canonical_plaintext, plaintext_lift, ephemeral, error_zero, error_one| {
            if canonical_plaintext.len() != profile.ring_degree
                || plaintext_lift.coefficients.len()
                    != profile
                        .ring_degree
                        .checked_mul(profile.moduli.len())
                        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                || ephemeral.coefficients.len() != profile.ring_degree
                || error_zero.coefficients.len() != profile.ring_degree
                || error_one.coefficients.len() != profile.ring_degree
            {
                return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
            }
            Ok(())
        },
    )?;

    let mut receipt = VerifiedZkAmsPhase23NativeBgvOpeningV1 {
        key_digest: key.digest(),
        layout_digest: layout.digest,
        plaintext_digest: plaintext.digest,
        ciphertext_digest: ciphertext.digest(),
        rns_binding_digest,
        digest: [0; 32],
    };
    receipt.digest = native_bgv_opening_receipt_digest_v1(&receipt);
    receipt.validate_for(key, layout, plaintext, ciphertext)?;
    Ok(receipt)
}

fn native_bgv_opening_receipt_digest_v1(
    receipt: &VerifiedZkAmsPhase23NativeBgvOpeningV1,
) -> [u8; 32] {
    let mut frame = Vec::with_capacity(NATIVE_BGV_OPENING_RECEIPT_DOMAIN_V1.len() + 1 + 5 * 32);
    frame.extend_from_slice(NATIVE_BGV_OPENING_RECEIPT_DOMAIN_V1);
    frame.push(RNS_LINK_VERSION_V1);
    frame.extend_from_slice(&receipt.key_digest);
    frame.extend_from_slice(&receipt.layout_digest);
    frame.extend_from_slice(&receipt.plaintext_digest);
    frame.extend_from_slice(&receipt.ciphertext_digest);
    frame.extend_from_slice(&receipt.rns_binding_digest);
    keccak256(&frame)
}

// Compile-time API guards: mutable readiness and KAT evidence have no place in
// either production context construction or production challenge derivation.
type RnsLinkContextConstructorV1 = fn(
    [u8; 32],
    [u8; 32],
    [u8; 32],
    [u8; 32],
    [u8; 32],
    [u8; 32],
    [u8; 32],
) -> Result<ZkAmsPhase23RnsLinkContextV1, ZkAmsMkheErrorV1>;
type RnsLinkChallengeConstructorV1 =
    fn(
        &ZkAmsPhase23RnsLinkPrechallengeV1,
    ) -> Result<ZkAmsPhase23RnsLinkChallengeSetV1, ZkAmsMkheErrorV1>;
const RNS_LINK_CONTEXT_SIGNATURE_GUARD_V1: RnsLinkContextConstructorV1 =
    ZkAmsPhase23RnsLinkContextV1::new;
const RNS_LINK_CHALLENGE_SIGNATURE_GUARD_V1: RnsLinkChallengeConstructorV1 =
    derive_release_evaluation_points_v1;

fn derive_evaluation_points_for_moduli_v1(
    prechallenge: &ZkAmsPhase23RnsLinkPrechallengeV1,
    moduli: &[u64],
) -> Result<ZkAmsPhase23RnsLinkChallengeSetV1, ZkAmsMkheErrorV1> {
    if moduli.is_empty() || moduli.len() > 64 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let point_count = moduli
        .len()
        .checked_mul(RNS_LINK_EVALUATIONS_PER_LIMB_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut points = Vec::new();
    points
        .try_reserve_exact(point_count)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;

    let mut modulus_frame = Vec::with_capacity(8 + moduli.len() * 8);
    modulus_frame.extend_from_slice(CHALLENGE_SET_DOMAIN_V1);
    modulus_frame.push(RNS_LINK_VERSION_V1);
    modulus_frame
        .push(u8::try_from(moduli.len()).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?);
    for (limb_index, &modulus) in moduli.iter().enumerate() {
        if !(3..(1_u64 << 62)).contains(&modulus)
            || modulus % 2 == 0
            || moduli[..limb_index].contains(&modulus)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        modulus_frame.extend_from_slice(&modulus.to_be_bytes());
    }
    let modulus_set_digest = keccak256(&modulus_frame);

    for (limb_index, &modulus) in moduli.iter().enumerate() {
        let limb_start = points.len();
        for repetition in 0..RNS_LINK_EVALUATIONS_PER_LIMB_V1 {
            let prior_values: Vec<u64> = points[limb_start..]
                .iter()
                .map(|point: &ZkAmsPhase23RnsLinkEvaluationPointV1| point.value)
                .collect();
            let value = sample_canonical_nonzero_distinct_v1(modulus, &prior_values, |attempt| {
                let mut frame = Vec::with_capacity(EVALUATION_POINT_DOMAIN_V1.len() + 90);
                frame.extend_from_slice(EVALUATION_POINT_DOMAIN_V1);
                frame.push(RNS_LINK_VERSION_V1);
                frame.extend_from_slice(&prechallenge.transcript_digest);
                frame.extend_from_slice(&prechallenge.context_digest);
                frame.extend_from_slice(&prechallenge.ordered_commitment_root);
                frame.extend_from_slice(&modulus_set_digest);
                frame.push(u8::try_from(limb_index).expect("at most 64 limbs"));
                frame.push(u8::try_from(repetition).expect("five repetitions"));
                frame.extend_from_slice(&modulus.to_be_bytes());
                frame.extend_from_slice(&attempt.to_be_bytes());
                let uniform: [u8; 8] = shake256(&frame, 8)
                    .try_into()
                    .expect("fixed SHAKE output length");
                u64::from_be_bytes(uniform)
            })?;
            points.push(ZkAmsPhase23RnsLinkEvaluationPointV1 {
                limb_index: u8::try_from(limb_index)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                repetition: u8::try_from(repetition)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                modulus,
                value,
            });
        }
    }

    let mut digest_frame =
        Vec::with_capacity(CHALLENGE_SET_DOMAIN_V1.len() + 70 + points.len() * 18);
    digest_frame.extend_from_slice(CHALLENGE_SET_DOMAIN_V1);
    digest_frame.push(RNS_LINK_VERSION_V1);
    digest_frame.extend_from_slice(&prechallenge.transcript_digest);
    digest_frame.extend_from_slice(&modulus_set_digest);
    for point in &points {
        digest_frame.push(point.limb_index);
        digest_frame.push(point.repetition);
        digest_frame.extend_from_slice(&point.modulus.to_be_bytes());
        digest_frame.extend_from_slice(&point.value.to_be_bytes());
    }
    let digest = keccak256(&digest_frame);
    Ok(ZkAmsPhase23RnsLinkChallengeSetV1 {
        prechallenge_digest: prechallenge.transcript_digest,
        modulus_set_digest,
        points,
        digest,
    })
}

/// Unbiased reduction of a 64-bit Fiat--Shamir word. Zero and points already
/// used for the same limb are rejected under an exact, governed retry ceiling.
fn sample_canonical_nonzero_distinct_v1<F>(
    modulus: u64,
    prior_values: &[u64],
    mut sample_word: F,
) -> Result<u64, ZkAmsMkheErrorV1>
where
    F: FnMut(u16) -> u64,
{
    if !(3..(1_u64 << 62)).contains(&modulus)
        || modulus % 2 == 0
        || prior_values
            .iter()
            .any(|value| *value == 0 || *value >= modulus)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    // `-q mod q` in wrapping u64 arithmetic is exactly `2^64 mod q`.
    let rejection_threshold = modulus.wrapping_neg() % modulus;
    for attempt in 0..RNS_LINK_REJECTION_ATTEMPTS_V1 {
        let word = sample_word(
            u16::try_from(attempt).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        );
        if word < rejection_threshold {
            continue;
        }
        let value = word % modulus;
        if value != 0 && !prior_values.contains(&value) {
            return Ok(value);
        }
    }
    Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
}

/// Source-level guard: every production RNS-Link secret vector is the same
/// audited owner used by the native T256 Bulletproof backend. A raw `Vec` is
/// intentionally not accepted anywhere in the production witness container.
type AuditedRnsLinkSecretScalarsV1 = ZeroizingT256ScalarVecV1;

/// Move-only owner for every secret table retained by an RNS-Link prover.
/// The field types are the compile-time erasure guard; test-only integer oracle
/// vectors below are not part of this production witness boundary.
struct ZkAmsPhase23RnsLinkWitnessSecretsV1 {
    bit_planes: AuditedRnsLinkSecretScalarsV1,
    small_openings: AuditedRnsLinkSecretScalarsV1,
    packing_trace: AuditedRnsLinkSecretScalarsV1,
    radix_carries: AuditedRnsLinkSecretScalarsV1,
    negacyclic_quotients: AuditedRnsLinkSecretScalarsV1,
    hyrax_blindings: AuditedRnsLinkSecretScalarsV1,
}

impl fmt::Debug for ZkAmsPhase23RnsLinkWitnessSecretsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ZkAmsPhase23RnsLinkWitnessSecretsV1")
            .field("bit_planes", &"[REDACTED]")
            .field("small_openings", &"[REDACTED]")
            .field("packing_trace", &"[REDACTED]")
            .field("radix_carries", &"[REDACTED]")
            .field("negacyclic_quotients", &"[REDACTED]")
            .field("hyrax_blindings", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
impl ZkAmsPhase23RnsLinkWitnessSecretsV1 {
    fn test_fixture() -> Self {
        let secret =
            || AuditedRnsLinkSecretScalarsV1::new(vec![Scalar::from_u64(7), -Scalar::from_u64(9)]);
        Self {
            bit_planes: secret(),
            small_openings: secret(),
            packing_trace: secret(),
            radix_carries: secret(),
            negacyclic_quotients: secret(),
            hyrax_blindings: secret(),
        }
    }
}

/// Generator basis for one logarithmic RNS-Link inner-product opening.
#[derive(Clone, Debug)]
struct ZkAmsPhase23RnsLinkIpaKeyV1 {
    generators: Vec<VegaT256PointV1>,
    evaluation_generator: VegaT256PointV1,
    digest: [u8; 32],
}

impl ZkAmsPhase23RnsLinkIpaKeyV1 {
    fn derive(vector_len: usize) -> Result<Self, ZkAmsMkheErrorV1> {
        if vector_len < 2
            || vector_len > RNS_LINK_IPA_MAX_VECTOR_LEN_V1
            || !vector_len.is_power_of_two()
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        #[cfg(test)]
        RNS_LINK_IPA_KEY_DERIVATIONS_V1.with(|count| count.set(count.get() + 1));
        let mut label = Vec::with_capacity(IPA_GENERATOR_LABEL_V1.len() + 2);
        label.extend_from_slice(IPA_GENERATOR_LABEL_V1);
        label.extend_from_slice(
            &u16::try_from(vector_len)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        let mut points = derive_t256_generators_v1(
            &label,
            vector_len
                .checked_add(1)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let evaluation_generator = points.pop().ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if points.len() != vector_len
            || evaluation_generator.is_identity()
            || points.iter().any(|point| point.is_identity())
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let mut digest_frame = Vec::with_capacity(IPA_GENERATOR_LABEL_V1.len() + 70 * vector_len);
        digest_frame.extend_from_slice(IPA_GENERATOR_LABEL_V1);
        digest_frame.extend_from_slice(
            &u16::try_from(vector_len)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        for point in &points {
            digest_frame.extend_from_slice(
                &point
                    .to_non_identity_wire_bytes()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
            );
        }
        digest_frame.extend_from_slice(
            &evaluation_generator
                .to_non_identity_wire_bytes()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
        );
        Ok(Self {
            generators: points,
            evaluation_generator,
            digest: keccak256(&digest_frame),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ZkAmsPhase23RnsLinkIpaStatementV1 {
    relation_context_digest: [u8; 32],
    key_digest: [u8; 32],
    vector_len: u16,
    commitment: VegaT256PointV1,
    evaluation: Scalar,
}

impl ZkAmsPhase23RnsLinkIpaStatementV1 {
    fn validate(&self, key: &ZkAmsPhase23RnsLinkIpaKeyV1) -> Result<(), ZkAmsMkheErrorV1> {
        if !is_nonzero_digest(self.relation_context_digest)
            || self.key_digest != key.digest
            || usize::from(self.vector_len) != key.generators.len()
            || self.commitment.is_identity()
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ZkAmsPhase23RnsLinkIpaProofV1 {
    left: Vec<VegaT256PointV1>,
    right: Vec<VegaT256PointV1>,
    final_witness: Scalar,
}

fn rns_link_secret_inner_product_v1(
    left: &[Scalar],
    right: &[Scalar],
) -> Result<ZeroizingT256ScalarCopyV1, ZkAmsMkheErrorV1> {
    if left.len() != right.len() || left.is_empty() {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(ZeroizingT256ScalarCopyV1::new(
        left.iter()
            .copied()
            .zip(right.iter().copied())
            .fold(Scalar::zero(), |sum, (left, right)| sum + left * right),
    ))
}

fn rns_link_secret_msm_v1(
    scalars: &[Scalar],
    points: &[VegaT256PointV1],
) -> Result<VegaT256PointV1, ZkAmsMkheErrorV1> {
    if scalars.len() != points.len() || scalars.is_empty() {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(scalars
        .iter()
        .copied()
        .zip(points.iter().copied())
        .fold(VegaT256PointV1::identity(), |sum, (scalar, point)| {
            sum + point.mul_scalar(scalar)
        }))
}

fn rns_link_squeeze_nonzero_v1(
    transcript: &mut VegaTranscriptV1,
    label: &'static [u8],
) -> Result<Scalar, ZkAmsMkheErrorV1> {
    for _ in 0..RNS_LINK_IPA_CHALLENGE_RETRIES_V1 {
        let challenge = transcript
            .squeeze(label)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if !challenge.is_zero() {
            return Ok(challenge);
        }
    }
    Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
}

fn absorb_rns_link_ipa_round_v1(
    transcript: &mut VegaTranscriptV1,
    left: VegaT256PointV1,
    right: VegaT256PointV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if left.is_identity() || right.is_identity() {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    transcript
        .absorb_raw(
            IPA_LEFT_LABEL_V1,
            &left
                .to_transcript_bytes()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
        )
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    transcript
        .absorb_raw(
            IPA_RIGHT_LABEL_V1,
            &right
                .to_transcript_bytes()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
        )
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)
}

fn prove_rns_link_ipa_v1(
    key: &ZkAmsPhase23RnsLinkIpaKeyV1,
    statement: &ZkAmsPhase23RnsLinkIpaStatementV1,
    mut witness: AuditedRnsLinkSecretScalarsV1,
    public_weights: &[Scalar],
    transcript: &mut VegaTranscriptV1,
) -> Result<ZkAmsPhase23RnsLinkIpaProofV1, ZkAmsMkheErrorV1> {
    statement.validate(key)?;
    if witness.len() != key.generators.len() || public_weights.len() != witness.len() {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    if rns_link_secret_msm_v1(witness.as_slice(), &key.generators)? != statement.commitment
        || rns_link_secret_inner_product_v1(witness.as_slice(), public_weights)?.get()
            != statement.evaluation
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    transcript
        .domain_separator(IPA_TRANSCRIPT_DOMAIN_V1)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    transcript
        .absorb_scalar(IPA_EVALUATION_LABEL_V1, statement.evaluation)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;

    let mut generators = key.generators.clone();
    let mut weights = public_weights.to_vec();
    let mut left_rounds = Vec::with_capacity(witness.len().ilog2() as usize);
    let mut right_rounds = Vec::with_capacity(witness.len().ilog2() as usize);
    while witness.len() > 1 {
        let half = witness.len() / 2;
        let (a_left, a_right) = witness.as_slice().split_at(half);
        let (b_left, b_right) = weights.split_at(half);
        let (g_left, g_right) = generators.split_at(half);
        let c_left = rns_link_secret_inner_product_v1(a_left, b_right)?;
        let c_right = rns_link_secret_inner_product_v1(a_right, b_left)?;
        let left = rns_link_secret_msm_v1(a_left, g_right)?
            + key.evaluation_generator.mul_scalar(c_left.get());
        let right = rns_link_secret_msm_v1(a_right, g_left)?
            + key.evaluation_generator.mul_scalar(c_right.get());
        absorb_rns_link_ipa_round_v1(transcript, left, right)?;
        let challenge = rns_link_squeeze_nonzero_v1(transcript, IPA_CHALLENGE_LABEL_V1)?;
        let inverse = challenge
            .inverse()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;

        let mut folded_witness = AuditedRnsLinkSecretScalarsV1::with_capacity(half);
        let mut folded_weights = Vec::with_capacity(half);
        let mut folded_generators = Vec::with_capacity(half);
        for index in 0..half {
            folded_witness.push(challenge * a_left[index] + inverse * a_right[index]);
            folded_weights.push(inverse * b_left[index] + challenge * b_right[index]);
            folded_generators
                .push(g_left[index].mul_scalar(inverse) + g_right[index].mul_scalar(challenge));
        }
        witness = folded_witness;
        weights = folded_weights;
        generators = folded_generators;
        left_rounds.push(left);
        right_rounds.push(right);
    }
    if left_rounds.len() > RNS_LINK_IPA_MAX_ROUNDS_V1 {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    Ok(ZkAmsPhase23RnsLinkIpaProofV1 {
        left: left_rounds,
        right: right_rounds,
        final_witness: witness.as_slice()[0],
    })
}

fn verify_rns_link_ipa_v1(
    key: &ZkAmsPhase23RnsLinkIpaKeyV1,
    statement: &ZkAmsPhase23RnsLinkIpaStatementV1,
    public_weights: &[Scalar],
    proof: &ZkAmsPhase23RnsLinkIpaProofV1,
    transcript: &mut VegaTranscriptV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    statement.validate(key)?;
    let expected_rounds = usize::from(statement.vector_len).ilog2() as usize;
    if public_weights.len() != key.generators.len()
        || proof.left.len() != expected_rounds
        || proof.right.len() != expected_rounds
        || proof.left.len() > RNS_LINK_IPA_MAX_ROUNDS_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    transcript
        .domain_separator(IPA_TRANSCRIPT_DOMAIN_V1)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    transcript
        .absorb_scalar(IPA_EVALUATION_LABEL_V1, statement.evaluation)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let mut commitment =
        statement.commitment + key.evaluation_generator.mul_scalar(statement.evaluation);
    let mut generators = key.generators.clone();
    let mut weights = public_weights.to_vec();
    for (&left, &right) in proof.left.iter().zip(&proof.right) {
        absorb_rns_link_ipa_round_v1(transcript, left, right)?;
        let challenge = rns_link_squeeze_nonzero_v1(transcript, IPA_CHALLENGE_LABEL_V1)?;
        let inverse = challenge
            .inverse()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        commitment =
            left.mul_scalar(challenge.square()) + commitment + right.mul_scalar(inverse.square());
        let half = generators.len() / 2;
        if half == 0 || weights.len() != generators.len() {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let (g_left, g_right) = generators.split_at(half);
        let (b_left, b_right) = weights.split_at(half);
        let mut folded_generators = Vec::with_capacity(half);
        let mut folded_weights = Vec::with_capacity(half);
        for index in 0..half {
            folded_generators
                .push(g_left[index].mul_scalar(inverse) + g_right[index].mul_scalar(challenge));
            folded_weights.push(inverse * b_left[index] + challenge * b_right[index]);
        }
        generators = folded_generators;
        weights = folded_weights;
    }
    if generators.len() != 1 || weights.len() != 1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let expected = generators[0].mul_scalar(proof.final_witness)
        + key
            .evaluation_generator
            .mul_scalar(proof.final_witness * weights[0]);
    if commitment != expected {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ZkAmsPhase23RnsLinkBitnessStatementV1 {
    relation_context_digest: [u8; 32],
    value_count: u16,
    key_digest: [u8; 32],
    commitment: VegaT256PointV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ZkAmsPhase23RnsLinkBitnessProofV1 {
    sumcheck: SumcheckProof,
    evaluation: Scalar,
    ipa: ZkAmsPhase23RnsLinkIpaProofV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RnsLinkBitnessCodecShapeV1 {
    vector_len: usize,
    sumcheck_rounds: usize,
    sumcheck_coefficients: usize,
    ipa_rounds: usize,
    sumcheck_bytes: usize,
    evaluation_bytes: usize,
    ipa_left_bytes: usize,
    ipa_right_bytes: usize,
    final_witness_bytes: usize,
    body_bytes: usize,
    total_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RnsLinkBitnessCodecLayoutV1 {
    shape: RnsLinkBitnessCodecShapeV1,
    sumcheck_offset: usize,
    evaluation_offset: usize,
    ipa_left_offset: usize,
    ipa_right_offset: usize,
    final_witness_offset: usize,
}

fn rns_link_bitness_codec_shape_v1(
    statement: &ZkAmsPhase23RnsLinkBitnessStatementV1,
) -> Result<RnsLinkBitnessCodecShapeV1, ZkAmsMkheErrorV1> {
    let value_count = usize::from(statement.value_count);
    if !is_nonzero_digest(statement.relation_context_digest)
        || !is_nonzero_digest(statement.key_digest)
        || statement.commitment.is_identity()
        || value_count < 2
        || value_count > RNS_LINK_BITNESS_MAX_VALUES_V1
        || !value_count.is_power_of_two()
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let sumcheck_rounds = value_count.ilog2() as usize;
    let vector_len = value_count
        .checked_add(1)
        .and_then(usize::checked_next_power_of_two)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let ipa_rounds = vector_len.ilog2() as usize;
    if sumcheck_rounds == 0
        || sumcheck_rounds > RNS_LINK_BITNESS_MAX_SUMCHECK_ROUNDS_V1
        || vector_len > RNS_LINK_IPA_MAX_VECTOR_LEN_V1
        || ipa_rounds > RNS_LINK_IPA_MAX_ROUNDS_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let sumcheck_coefficients = sumcheck_rounds
        .checked_mul(RNS_LINK_BITNESS_SUMCHECK_DEGREE_V1)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let sumcheck_bytes = sumcheck_coefficients
        .checked_mul(RNS_LINK_SCALAR_WIRE_BYTES_V1)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let evaluation_bytes = RNS_LINK_SCALAR_WIRE_BYTES_V1;
    let ipa_left_bytes = ipa_rounds
        .checked_mul(RNS_LINK_POINT_WIRE_BYTES_V1)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let ipa_right_bytes = ipa_left_bytes;
    let final_witness_bytes = RNS_LINK_SCALAR_WIRE_BYTES_V1;
    let body_bytes = [
        sumcheck_bytes,
        evaluation_bytes,
        ipa_left_bytes,
        ipa_right_bytes,
        final_witness_bytes,
    ]
    .into_iter()
    .try_fold(0_usize, usize::checked_add)
    .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let total_bytes = RNS_LINK_BITNESS_CODEC_HEADER_BYTES_V1
        .checked_add(body_bytes)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    if body_bytes > RNS_LINK_BITNESS_CODEC_MAX_BODY_BYTES_V1
        || total_bytes > RNS_LINK_BITNESS_CODEC_MAX_BYTES_V1
        || total_bytes > ZK_AMS_MKHE_MAX_PROOF_BYTES_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(RnsLinkBitnessCodecShapeV1 {
        vector_len,
        sumcheck_rounds,
        sumcheck_coefficients,
        ipa_rounds,
        sumcheck_bytes,
        evaluation_bytes,
        ipa_left_bytes,
        ipa_right_bytes,
        final_witness_bytes,
        body_bytes,
        total_bytes,
    })
}

fn rns_link_wire_array_v1<const N: usize>(
    bytes: &[u8],
    offset: usize,
) -> Result<[u8; N], ZkAmsMkheErrorV1> {
    let end = offset
        .checked_add(N)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    bytes
        .get(offset..end)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
        .try_into()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)
}

fn rns_link_wire_u16_v1(bytes: &[u8], offset: usize) -> Result<u16, ZkAmsMkheErrorV1> {
    Ok(u16::from_be_bytes(rns_link_wire_array_v1(bytes, offset)?))
}

fn rns_link_wire_u64_v1(bytes: &[u8], offset: usize) -> Result<u64, ZkAmsMkheErrorV1> {
    Ok(u64::from_be_bytes(rns_link_wire_array_v1(bytes, offset)?))
}

fn rns_link_wire_length_v1(bytes: &[u8], offset: usize) -> Result<usize, ZkAmsMkheErrorV1> {
    usize::try_from(rns_link_wire_u64_v1(bytes, offset)?)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)
}

/// Allocation-light structural pass. It completes exact length arithmetic and
/// canonical scalar/point validation before proof vectors or IPA generators
/// can be allocated.
fn preflight_rns_link_bitness_wire_v1(
    statement: &ZkAmsPhase23RnsLinkBitnessStatementV1,
    bytes: &[u8],
) -> Result<RnsLinkBitnessCodecLayoutV1, ZkAmsMkheErrorV1> {
    if bytes.len() < RNS_LINK_BITNESS_CODEC_HEADER_BYTES_V1
        || bytes.len() > RNS_LINK_BITNESS_CODEC_MAX_BYTES_V1
        || bytes.len() > ZK_AMS_MKHE_MAX_PROOF_BYTES_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let shape = rns_link_bitness_codec_shape_v1(statement)?;
    if bytes.get(..8) != Some(RNS_LINK_BITNESS_CODEC_MAGIC_V1.as_slice())
        || bytes[8] != RNS_LINK_VERSION_V1
        || bytes[9] != 0
        || usize::from(rns_link_wire_u16_v1(bytes, 10)?) != RNS_LINK_BITNESS_CODEC_HEADER_BYTES_V1
        || rns_link_wire_array_v1::<32>(bytes, BITNESS_CODEC_MANIFEST_OFFSET_V1)?
            != immutable_algorithm_manifest_digest_v1()?
        || usize::from(rns_link_wire_u16_v1(
            bytes,
            BITNESS_CODEC_VALUE_COUNT_OFFSET_V1,
        )?) != usize::from(statement.value_count)
        || usize::from(bytes[BITNESS_CODEC_SUMCHECK_ROUNDS_OFFSET_V1]) != shape.sumcheck_rounds
        || usize::from(bytes[BITNESS_CODEC_SUMCHECK_DEGREE_OFFSET_V1])
            != RNS_LINK_BITNESS_SUMCHECK_DEGREE_V1
        || usize::from(rns_link_wire_u16_v1(
            bytes,
            BITNESS_CODEC_SUMCHECK_COEFFICIENT_COUNT_OFFSET_V1,
        )?) != shape.sumcheck_coefficients
        || usize::from(bytes[BITNESS_CODEC_IPA_ROUNDS_OFFSET_V1]) != shape.ipa_rounds
        || usize::from(bytes[BITNESS_CODEC_IPA_LEFT_COUNT_OFFSET_V1]) != shape.ipa_rounds
        || usize::from(bytes[BITNESS_CODEC_IPA_RIGHT_COUNT_OFFSET_V1]) != shape.ipa_rounds
        || usize::from(bytes[BITNESS_CODEC_POINT_BYTES_OFFSET_V1]) != RNS_LINK_POINT_WIRE_BYTES_V1
        || usize::from(bytes[BITNESS_CODEC_SCALAR_BYTES_OFFSET_V1]) != RNS_LINK_SCALAR_WIRE_BYTES_V1
        || bytes[BITNESS_CODEC_RESERVED_OFFSET_V1] != 0
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }

    let encoded_lengths = [
        rns_link_wire_length_v1(bytes, BITNESS_CODEC_SUMCHECK_BYTES_OFFSET_V1)?,
        rns_link_wire_length_v1(bytes, BITNESS_CODEC_EVALUATION_BYTES_OFFSET_V1)?,
        rns_link_wire_length_v1(bytes, BITNESS_CODEC_IPA_LEFT_BYTES_OFFSET_V1)?,
        rns_link_wire_length_v1(bytes, BITNESS_CODEC_IPA_RIGHT_BYTES_OFFSET_V1)?,
        rns_link_wire_length_v1(bytes, BITNESS_CODEC_FINAL_WITNESS_BYTES_OFFSET_V1)?,
    ];
    let encoded_body_bytes = encoded_lengths
        .into_iter()
        .try_fold(0_usize, usize::checked_add)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let declared_body_bytes = rns_link_wire_length_v1(bytes, BITNESS_CODEC_BODY_BYTES_OFFSET_V1)?;
    let declared_total_bytes = rns_link_wire_length_v1(bytes, BITNESS_CODEC_TOTAL_BYTES_OFFSET_V1)?;
    let computed_total_bytes = RNS_LINK_BITNESS_CODEC_HEADER_BYTES_V1
        .checked_add(declared_body_bytes)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    if encoded_lengths
        != [
            shape.sumcheck_bytes,
            shape.evaluation_bytes,
            shape.ipa_left_bytes,
            shape.ipa_right_bytes,
            shape.final_witness_bytes,
        ]
        || encoded_body_bytes != declared_body_bytes
        || declared_body_bytes != shape.body_bytes
        || computed_total_bytes != declared_total_bytes
        || declared_total_bytes != shape.total_bytes
        || declared_total_bytes != bytes.len()
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }

    let sumcheck_offset = RNS_LINK_BITNESS_CODEC_HEADER_BYTES_V1;
    let evaluation_offset = sumcheck_offset
        .checked_add(shape.sumcheck_bytes)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let ipa_left_offset = evaluation_offset
        .checked_add(shape.evaluation_bytes)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let ipa_right_offset = ipa_left_offset
        .checked_add(shape.ipa_left_bytes)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let final_witness_offset = ipa_right_offset
        .checked_add(shape.ipa_right_bytes)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    if final_witness_offset
        .checked_add(shape.final_witness_bytes)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
        != bytes.len()
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }

    for index in 0..shape.sumcheck_coefficients {
        let offset = sumcheck_offset
            .checked_add(
                index
                    .checked_mul(RNS_LINK_SCALAR_WIRE_BYTES_V1)
                    .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
            )
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        Scalar::from_be_bytes_exact(rns_link_wire_array_v1(bytes, offset)?)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    }
    Scalar::from_be_bytes_exact(rns_link_wire_array_v1(bytes, evaluation_offset)?)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    for ordinal in 0..shape.ipa_rounds {
        let delta = ordinal
            .checked_mul(RNS_LINK_POINT_WIRE_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let left_offset = ipa_left_offset
            .checked_add(delta)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let right_offset = ipa_right_offset
            .checked_add(delta)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        VegaT256PointV1::from_non_identity_wire_bytes_exact(&rns_link_wire_array_v1::<
            RNS_LINK_POINT_WIRE_BYTES_V1,
        >(bytes, left_offset)?)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        VegaT256PointV1::from_non_identity_wire_bytes_exact(&rns_link_wire_array_v1::<
            RNS_LINK_POINT_WIRE_BYTES_V1,
        >(bytes, right_offset)?)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    }
    Scalar::from_be_bytes_exact(rns_link_wire_array_v1(bytes, final_witness_offset)?)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    Ok(RnsLinkBitnessCodecLayoutV1 {
        shape,
        sumcheck_offset,
        evaluation_offset,
        ipa_left_offset,
        ipa_right_offset,
        final_witness_offset,
    })
}

fn write_rns_link_wire_u16_v1(
    bytes: &mut [u8],
    offset: usize,
    value: usize,
) -> Result<(), ZkAmsMkheErrorV1> {
    let encoded = u16::try_from(value)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
        .to_be_bytes();
    let end = offset
        .checked_add(encoded.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    bytes
        .get_mut(offset..end)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
        .copy_from_slice(&encoded);
    Ok(())
}

fn write_rns_link_wire_u64_v1(
    bytes: &mut [u8],
    offset: usize,
    value: usize,
) -> Result<(), ZkAmsMkheErrorV1> {
    let encoded = u64::try_from(value)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
        .to_be_bytes();
    let end = offset
        .checked_add(encoded.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    bytes
        .get_mut(offset..end)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
        .copy_from_slice(&encoded);
    Ok(())
}

fn encode_rns_link_bitness_proof_v1(
    statement: &ZkAmsPhase23RnsLinkBitnessStatementV1,
    proof: &ZkAmsPhase23RnsLinkBitnessProofV1,
) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
    let shape = rns_link_bitness_codec_shape_v1(statement)?;
    if proof.sumcheck.rounds.len() != shape.sumcheck_rounds
        || proof.ipa.left.len() != shape.ipa_rounds
        || proof.ipa.right.len() != shape.ipa_rounds
        || proof
            .sumcheck
            .rounds
            .iter()
            .any(|round| round.coefficients().len() != RNS_LINK_BITNESS_SUMCHECK_DEGREE_V1)
        || proof.ipa.left.iter().any(|point| point.is_identity())
        || proof.ipa.right.iter().any(|point| point.is_identity())
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut bytes = vec![0_u8; RNS_LINK_BITNESS_CODEC_HEADER_BYTES_V1];
    bytes[..8].copy_from_slice(&RNS_LINK_BITNESS_CODEC_MAGIC_V1);
    bytes[8] = RNS_LINK_VERSION_V1;
    bytes[9] = 0;
    write_rns_link_wire_u16_v1(&mut bytes, 10, RNS_LINK_BITNESS_CODEC_HEADER_BYTES_V1)?;
    bytes[BITNESS_CODEC_MANIFEST_OFFSET_V1..BITNESS_CODEC_VALUE_COUNT_OFFSET_V1]
        .copy_from_slice(&immutable_algorithm_manifest_digest_v1()?);
    write_rns_link_wire_u16_v1(
        &mut bytes,
        BITNESS_CODEC_VALUE_COUNT_OFFSET_V1,
        usize::from(statement.value_count),
    )?;
    bytes[BITNESS_CODEC_SUMCHECK_ROUNDS_OFFSET_V1] = u8::try_from(shape.sumcheck_rounds)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    bytes[BITNESS_CODEC_SUMCHECK_DEGREE_OFFSET_V1] =
        u8::try_from(RNS_LINK_BITNESS_SUMCHECK_DEGREE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    write_rns_link_wire_u16_v1(
        &mut bytes,
        BITNESS_CODEC_SUMCHECK_COEFFICIENT_COUNT_OFFSET_V1,
        shape.sumcheck_coefficients,
    )?;
    bytes[BITNESS_CODEC_IPA_ROUNDS_OFFSET_V1] =
        u8::try_from(shape.ipa_rounds).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    bytes[BITNESS_CODEC_IPA_LEFT_COUNT_OFFSET_V1] = bytes[BITNESS_CODEC_IPA_ROUNDS_OFFSET_V1];
    bytes[BITNESS_CODEC_IPA_RIGHT_COUNT_OFFSET_V1] = bytes[BITNESS_CODEC_IPA_ROUNDS_OFFSET_V1];
    bytes[BITNESS_CODEC_POINT_BYTES_OFFSET_V1] = u8::try_from(RNS_LINK_POINT_WIRE_BYTES_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    bytes[BITNESS_CODEC_SCALAR_BYTES_OFFSET_V1] = u8::try_from(RNS_LINK_SCALAR_WIRE_BYTES_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    write_rns_link_wire_u64_v1(
        &mut bytes,
        BITNESS_CODEC_SUMCHECK_BYTES_OFFSET_V1,
        shape.sumcheck_bytes,
    )?;
    write_rns_link_wire_u64_v1(
        &mut bytes,
        BITNESS_CODEC_EVALUATION_BYTES_OFFSET_V1,
        shape.evaluation_bytes,
    )?;
    write_rns_link_wire_u64_v1(
        &mut bytes,
        BITNESS_CODEC_IPA_LEFT_BYTES_OFFSET_V1,
        shape.ipa_left_bytes,
    )?;
    write_rns_link_wire_u64_v1(
        &mut bytes,
        BITNESS_CODEC_IPA_RIGHT_BYTES_OFFSET_V1,
        shape.ipa_right_bytes,
    )?;
    write_rns_link_wire_u64_v1(
        &mut bytes,
        BITNESS_CODEC_FINAL_WITNESS_BYTES_OFFSET_V1,
        shape.final_witness_bytes,
    )?;
    write_rns_link_wire_u64_v1(
        &mut bytes,
        BITNESS_CODEC_BODY_BYTES_OFFSET_V1,
        shape.body_bytes,
    )?;
    write_rns_link_wire_u64_v1(
        &mut bytes,
        BITNESS_CODEC_TOTAL_BYTES_OFFSET_V1,
        shape.total_bytes,
    )?;
    bytes.reserve_exact(shape.body_bytes);
    for round in &proof.sumcheck.rounds {
        for coefficient in round.coefficients() {
            bytes.extend_from_slice(&coefficient.to_be_bytes());
        }
    }
    bytes.extend_from_slice(&proof.evaluation.to_be_bytes());
    for point in &proof.ipa.left {
        bytes.extend_from_slice(
            &point
                .to_non_identity_wire_bytes()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
        );
    }
    for point in &proof.ipa.right {
        bytes.extend_from_slice(
            &point
                .to_non_identity_wire_bytes()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
        );
    }
    bytes.extend_from_slice(&proof.ipa.final_witness.to_be_bytes());
    if bytes.len() != shape.total_bytes {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    preflight_rns_link_bitness_wire_v1(statement, &bytes)?;
    Ok(bytes)
}

fn decode_rns_link_bitness_proof_v1(
    statement: &ZkAmsPhase23RnsLinkBitnessStatementV1,
    bytes: &[u8],
) -> Result<ZkAmsPhase23RnsLinkBitnessProofV1, ZkAmsMkheErrorV1> {
    let layout = preflight_rns_link_bitness_wire_v1(statement, bytes)?;
    #[cfg(test)]
    RNS_LINK_CODEC_BODY_ALLOCATIONS_V1.with(|count| count.set(count.get() + 1));
    let mut rounds = Vec::with_capacity(layout.shape.sumcheck_rounds);
    let mut offset = layout.sumcheck_offset;
    for _ in 0..layout.shape.sumcheck_rounds {
        let mut coefficients = Vec::with_capacity(RNS_LINK_BITNESS_SUMCHECK_DEGREE_V1);
        for _ in 0..RNS_LINK_BITNESS_SUMCHECK_DEGREE_V1 {
            coefficients.push(
                Scalar::from_be_bytes_exact(rns_link_wire_array_v1(bytes, offset)?)
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
            );
            offset = offset
                .checked_add(RNS_LINK_SCALAR_WIRE_BYTES_V1)
                .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        }
        rounds.push(
            CompressedUnivariate::new(coefficients, RNS_LINK_BITNESS_SUMCHECK_DEGREE_V1)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
        );
    }
    if offset != layout.evaluation_offset {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let evaluation =
        Scalar::from_be_bytes_exact(rns_link_wire_array_v1(bytes, layout.evaluation_offset)?)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let mut left = Vec::with_capacity(layout.shape.ipa_rounds);
    let mut right = Vec::with_capacity(layout.shape.ipa_rounds);
    for ordinal in 0..layout.shape.ipa_rounds {
        let delta = ordinal
            .checked_mul(RNS_LINK_POINT_WIRE_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        left.push(
            VegaT256PointV1::from_non_identity_wire_bytes_exact(&rns_link_wire_array_v1::<
                RNS_LINK_POINT_WIRE_BYTES_V1,
            >(
                bytes,
                layout
                    .ipa_left_offset
                    .checked_add(delta)
                    .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
            )?)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
        );
        right.push(
            VegaT256PointV1::from_non_identity_wire_bytes_exact(&rns_link_wire_array_v1::<
                RNS_LINK_POINT_WIRE_BYTES_V1,
            >(
                bytes,
                layout
                    .ipa_right_offset
                    .checked_add(delta)
                    .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
            )?)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
        );
    }
    let final_witness =
        Scalar::from_be_bytes_exact(rns_link_wire_array_v1(bytes, layout.final_witness_offset)?)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    Ok(ZkAmsPhase23RnsLinkBitnessProofV1 {
        sumcheck: SumcheckProof::new(rounds),
        evaluation,
        ipa: ZkAmsPhase23RnsLinkIpaProofV1 {
            left,
            right,
            final_witness,
        },
    })
}

fn rns_link_bitness_transcript_v1(
    statement: &ZkAmsPhase23RnsLinkBitnessStatementV1,
) -> Result<VegaTranscriptV1, ZkAmsMkheErrorV1> {
    if !is_nonzero_digest(statement.relation_context_digest)
        || !is_nonzero_digest(statement.key_digest)
        || statement.commitment.is_identity()
        || usize::from(statement.value_count) < 2
        || usize::from(statement.value_count) > RNS_LINK_BITNESS_MAX_VALUES_V1
        || !usize::from(statement.value_count).is_power_of_two()
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut transcript = VegaTranscriptV1::new_neutron_nova();
    transcript
        .domain_separator(BITNESS_SUMCHECK_DOMAIN_V1)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    transcript
        .absorb_raw(
            BITNESS_ALGORITHM_LABEL_V1,
            &immutable_algorithm_manifest_digest_v1()?,
        )
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    transcript
        .absorb_raw(BITNESS_CONTEXT_LABEL_V1, &statement.relation_context_digest)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    transcript
        .absorb_raw(BITNESS_IPA_KEY_LABEL_V1, &statement.key_digest)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    transcript
        .absorb_raw(
            BITNESS_VALUE_COUNT_LABEL_V1,
            &statement.value_count.to_be_bytes(),
        )
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    transcript
        .absorb_raw(
            BITNESS_COMMITMENT_LABEL_V1,
            &statement
                .commitment
                .to_transcript_bytes()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
        )
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    Ok(transcript)
}

fn bind_rns_link_public_table_v1(
    table: &mut Vec<Scalar>,
    challenge: Scalar,
) -> Result<(), ZkAmsMkheErrorV1> {
    if table.len() < 2 || !table.len().is_power_of_two() {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let half = table.len() / 2;
    let (lower, upper) = table.split_at_mut(half);
    for index in 0..half {
        lower[index] += challenge * (upper[index] - lower[index]);
    }
    table.truncate(half);
    Ok(())
}

fn bind_rns_link_secret_table_v1(
    table: &mut AuditedRnsLinkSecretScalarsV1,
    challenge: Scalar,
) -> Result<(), ZkAmsMkheErrorV1> {
    if table.len() < 2 || !table.len().is_power_of_two() {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let half = table.len() / 2;
    let values = table.as_mut_slice();
    let (lower, upper) = values.split_at_mut(half);
    for index in 0..half {
        lower[index] += challenge * (upper[index] - lower[index]);
    }
    table.clear_and_truncate(half);
    Ok(())
}

fn interpolate_rns_link_cubic_v1(
    evaluations: [Scalar; 4],
) -> Result<[Scalar; 4], ZkAmsMkheErrorV1> {
    let two_inverse = Scalar::from_u64(2)
        .inverse()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let six_inverse = Scalar::from_u64(6)
        .inverse()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let constant = evaluations[0];
    let cubic = (evaluations[3] - Scalar::from_u64(3) * evaluations[2]
        + Scalar::from_u64(3) * evaluations[1]
        - evaluations[0])
        * six_inverse;
    let quadratic = (evaluations[2] - Scalar::from_u64(2) * evaluations[1] + evaluations[0])
        * two_inverse
        - Scalar::from_u64(3) * cubic;
    let linear = evaluations[1] - constant - quadratic - cubic;
    Ok([constant, linear, quadratic, cubic])
}

fn prove_rns_link_bitness_sumcheck_v1(
    mut bits: AuditedRnsLinkSecretScalarsV1,
    tau: &[Scalar],
    transcript: &mut VegaTranscriptV1,
) -> Result<(SumcheckProof, Vec<Scalar>, Scalar), ZkAmsMkheErrorV1> {
    let expected = 1_usize
        .checked_shl(
            u32::try_from(tau.len()).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if bits.len() != expected || expected > RNS_LINK_BITNESS_MAX_VALUES_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut bit_minus_one = AuditedRnsLinkSecretScalarsV1::with_capacity(bits.len());
    for bit in bits.as_slice().iter().copied() {
        bit_minus_one.push(bit - Scalar::one());
    }
    let mut equality = eq_evals(tau).map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let mut claim = Scalar::zero();
    let mut rounds = Vec::with_capacity(tau.len());
    let mut challenges = Vec::with_capacity(tau.len());
    while bits.len() > 1 {
        let half = bits.len() / 2;
        let (a_zero, a_one) = bits.as_slice().split_at(half);
        let (b_zero, b_one) = bit_minus_one.as_slice().split_at(half);
        let (eq_zero, eq_one) = equality.split_at(half);
        let mut evaluations = [Scalar::zero(); 4];
        for index in 0..half {
            let delta_a = a_one[index] - a_zero[index];
            let delta_b = b_one[index] - b_zero[index];
            let delta_eq = eq_one[index] - eq_zero[index];
            for (ordinal, evaluation) in evaluations.iter_mut().enumerate() {
                let point = Scalar::from_u64(ordinal as u64);
                *evaluation += (eq_zero[index] + point * delta_eq)
                    * (a_zero[index] + point * delta_a)
                    * (b_zero[index] + point * delta_b);
            }
        }
        if evaluations[0] + evaluations[1] != claim {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let coefficients = interpolate_rns_link_cubic_v1(evaluations)?;
        let compressed = CompressedUnivariate::new(
            vec![coefficients[0], coefficients[2], coefficients[3]],
            RNS_LINK_BITNESS_SUMCHECK_DEGREE_V1,
        )
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        transcript
            .absorb_univariate(
                BITNESS_SUMCHECK_POLYNOMIAL_LABEL_V1,
                compressed.coefficients(),
            )
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let challenge = transcript
            .squeeze(BITNESS_SUMCHECK_CHALLENGE_LABEL_V1)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        claim = coefficients
            .iter()
            .rev()
            .fold(Scalar::zero(), |value, coefficient| {
                value * challenge + *coefficient
            });
        bind_rns_link_secret_table_v1(&mut bits, challenge)?;
        bind_rns_link_secret_table_v1(&mut bit_minus_one, challenge)?;
        bind_rns_link_public_table_v1(&mut equality, challenge)?;
        rounds.push(compressed);
        challenges.push(challenge);
    }
    Ok((SumcheckProof::new(rounds), challenges, bits.as_slice()[0]))
}

fn verify_rns_link_bitness_sumcheck_v1(
    proof: &SumcheckProof,
    round_count: usize,
    transcript: &mut VegaTranscriptV1,
) -> Result<(Scalar, Vec<Scalar>), ZkAmsMkheErrorV1> {
    if round_count == 0
        || round_count > RNS_LINK_BITNESS_MAX_SUMCHECK_ROUNDS_V1
        || proof.rounds.len() != round_count
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut claim = Scalar::zero();
    let mut challenges = Vec::with_capacity(round_count);
    for round in &proof.rounds {
        if round.coefficients().len() != RNS_LINK_BITNESS_SUMCHECK_DEGREE_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let polynomial = decompress_univariate(round.coefficients(), claim)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        transcript
            .absorb_univariate(BITNESS_SUMCHECK_POLYNOMIAL_LABEL_V1, round.coefficients())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let challenge = transcript
            .squeeze(BITNESS_SUMCHECK_CHALLENGE_LABEL_V1)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        claim = evaluate_univariate(&polynomial, challenge)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        challenges.push(challenge);
    }
    Ok((claim, challenges))
}

fn prove_rns_link_bitness_v1(
    relation_context_digest: [u8; 32],
    bits: AuditedRnsLinkSecretScalarsV1,
    blinding: Scalar,
) -> Result<
    (
        ZkAmsPhase23RnsLinkBitnessStatementV1,
        ZkAmsPhase23RnsLinkBitnessProofV1,
    ),
    ZkAmsMkheErrorV1,
> {
    if !is_nonzero_digest(relation_context_digest)
        || bits.len() < 2
        || bits.len() > RNS_LINK_BITNESS_MAX_VALUES_V1
        || !bits.len().is_power_of_two()
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let value_count = bits.len();
    let vector_len = value_count
        .checked_add(1)
        .and_then(usize::checked_next_power_of_two)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let key = ZkAmsPhase23RnsLinkIpaKeyV1::derive(vector_len)?;
    let blinding = ZeroizingT256ScalarCopyV1::new(blinding);
    let mut ipa_witness = AuditedRnsLinkSecretScalarsV1::with_capacity(vector_len);
    for bit in bits.as_slice().iter().copied() {
        ipa_witness.push(bit);
    }
    while ipa_witness.len() + 1 < vector_len {
        ipa_witness.push(Scalar::zero());
    }
    ipa_witness.push(blinding.get());
    let commitment = rns_link_secret_msm_v1(ipa_witness.as_slice(), &key.generators)?;
    if commitment.is_identity() {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let statement = ZkAmsPhase23RnsLinkBitnessStatementV1 {
        relation_context_digest,
        value_count: u16::try_from(value_count)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        key_digest: key.digest,
        commitment,
    };
    let mut transcript = rns_link_bitness_transcript_v1(&statement)?;
    let round_count = value_count.ilog2() as usize;
    let mut tau = Vec::with_capacity(round_count);
    for _ in 0..round_count {
        tau.push(
            transcript
                .squeeze(BITNESS_TAU_CHALLENGE_LABEL_V1)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
        );
    }
    let (sumcheck, point, evaluation) =
        prove_rns_link_bitness_sumcheck_v1(bits, &tau, &mut transcript)?;
    let mut opening_weights = eq_evals(&point).map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    opening_weights.resize(vector_len, Scalar::zero());
    let ipa_statement = ZkAmsPhase23RnsLinkIpaStatementV1 {
        relation_context_digest,
        key_digest: key.digest,
        vector_len: u16::try_from(vector_len)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        commitment,
        evaluation,
    };
    let ipa = prove_rns_link_ipa_v1(
        &key,
        &ipa_statement,
        ipa_witness,
        &opening_weights,
        &mut transcript,
    )?;
    Ok((
        statement,
        ZkAmsPhase23RnsLinkBitnessProofV1 {
            sumcheck,
            evaluation,
            ipa,
        },
    ))
}

fn verify_rns_link_bitness_v1(
    expected_relation_context_digest: [u8; 32],
    statement: &ZkAmsPhase23RnsLinkBitnessStatementV1,
    proof: &ZkAmsPhase23RnsLinkBitnessProofV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if statement.relation_context_digest != expected_relation_context_digest {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let shape = rns_link_bitness_codec_shape_v1(statement)?;
    if proof.sumcheck.rounds.len() != shape.sumcheck_rounds
        || proof.ipa.left.len() != shape.ipa_rounds
        || proof.ipa.right.len() != shape.ipa_rounds
        || proof
            .sumcheck
            .rounds
            .iter()
            .any(|round| round.coefficients().len() != RNS_LINK_BITNESS_SUMCHECK_DEGREE_V1)
        || proof.ipa.left.iter().any(|point| point.is_identity())
        || proof.ipa.right.iter().any(|point| point.is_identity())
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let key = ZkAmsPhase23RnsLinkIpaKeyV1::derive(shape.vector_len)?;
    if statement.key_digest != key.digest {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut transcript = rns_link_bitness_transcript_v1(statement)?;
    let round_count = shape.sumcheck_rounds;
    let mut tau = Vec::with_capacity(round_count);
    for _ in 0..round_count {
        tau.push(
            transcript
                .squeeze(BITNESS_TAU_CHALLENGE_LABEL_V1)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
        );
    }
    let (final_claim, point) =
        verify_rns_link_bitness_sumcheck_v1(&proof.sumcheck, round_count, &mut transcript)?;
    let expected_final = eq_evaluate(&tau, &point)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?
        * proof.evaluation
        * (proof.evaluation - Scalar::one());
    if final_claim != expected_final {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut opening_weights = eq_evals(&point).map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    opening_weights.resize(shape.vector_len, Scalar::zero());
    let ipa_statement = ZkAmsPhase23RnsLinkIpaStatementV1 {
        relation_context_digest: expected_relation_context_digest,
        key_digest: key.digest,
        vector_len: u16::try_from(shape.vector_len)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        commitment: statement.commitment,
        evaluation: proof.evaluation,
    };
    verify_rns_link_ipa_v1(
        &key,
        &ipa_statement,
        &opening_weights,
        &proof.ipa,
        &mut transcript,
    )
}

fn verify_rns_link_bitness_wire_v1(
    expected_relation_context_digest: [u8; 32],
    statement: &ZkAmsPhase23RnsLinkBitnessStatementV1,
    proof_bytes: &[u8],
) -> Result<(), ZkAmsMkheErrorV1> {
    if statement.relation_context_digest != expected_relation_context_digest {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let proof = decode_rns_link_bitness_proof_v1(statement, proof_bytes)?;
    verify_rns_link_bitness_v1(expected_relation_context_digest, statement, &proof)
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use crate::vega::{
        bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1, derive_t256_generators_v1,
        sponge::keccak256,
    };

    use super::*;

    const TINY_N: usize = 8;
    const TINY_Q: u64 = 97;
    const TINY_P: u64 = 17;
    const TINY_R_ABS_BOUND: i64 = 1;
    const TINY_E_ABS_BOUND: i64 = 2;
    const TINY_H_ABS_BOUND: i64 = 8 * (TINY_Q as i64 - 1);
    const TINY_CARRY_ABS_BOUND: i64 = 32;

    struct TinyRnsLinkWitnessV1 {
        a: Vec<u64>,
        r: Vec<i64>,
        e: Vec<i64>,
        message: Vec<u64>,
        ciphertext: Vec<u64>,
        quotient: Vec<i64>,
        carries: Vec<i64>,
        logical_slots: [u64; 4],
        used_slots: usize,
        last_row: [u64; 4],
        last_row_used: usize,
        absent_chunk_bitmap: u8,
    }

    impl fmt::Debug for TinyRnsLinkWitnessV1 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("TinyRnsLinkWitnessV1")
                .field("a", &self.a)
                .field("r", &"[REDACTED]")
                .field("e", &"[REDACTED]")
                .field("message", &self.message)
                .field("ciphertext", &self.ciphertext)
                .field("quotient", &"[REDACTED]")
                .field("carries", &"[REDACTED]")
                .field("logical_slots", &self.logical_slots)
                .field("last_row", &self.last_row)
                .finish()
        }
    }

    fn digest(label: &[u8]) -> [u8; 32] {
        keccak256(label)
    }

    fn test_context(network_label: &[u8]) -> ZkAmsPhase23RnsLinkContextV1 {
        ZkAmsPhase23RnsLinkContextV1::new(
            digest(network_label),
            digest(b"statement"),
            digest(b"transcript"),
            digest(b"batch"),
            digest(b"roster"),
            digest(b"exact-direct-key-admission"),
            zk_ams_phase23_release_map_set_digest_v1().unwrap(),
        )
        .unwrap()
    }

    fn commitment_digests(
        family: ZkAmsPhase23RnsLinkFamilyV1,
        chunk_index: u16,
    ) -> ZkAmsPhase23RnsLinkCommitmentDigestsV1 {
        let tagged = |suffix: u8| {
            digest(&[
                b'c',
                family as u8,
                u8::try_from(chunk_index).unwrap(),
                suffix,
            ])
        };
        ZkAmsPhase23RnsLinkCommitmentDigestsV1 {
            // A family layout is common across all chunks.
            layout_digest: digest(&[b'l', family as u8]),
            ciphertext_digest: tagged(1),
            bit_planes_digest: tagged(2),
            small_openings_digest: tagged(3),
            packing_trace_digest: tagged(4),
            radix_carry_digest: tagged(5),
            negacyclic_quotient_digest: tagged(6),
            padding_digest: tagged(7),
        }
    }

    fn hyrax_point(index: usize) -> [u8; 33] {
        derive_t256_generators_v1(b"iroha.zk-ams.v1.rns-link-test-hyrax", 2).unwrap()[index]
            .to_non_identity_wire_bytes()
            .unwrap()
    }

    fn chunk_commitment(
        family: ZkAmsPhase23RnsLinkFamilyV1,
        chunk_index: u16,
    ) -> ZkAmsPhase23RnsLinkChunkCommitmentV1 {
        let logical_value_count = expected_logical_values_v1(family);
        let chunk_count = logical_value_count.div_ceil(ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1);
        let consumed = usize::from(chunk_index) * ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1;
        let used_slots = (logical_value_count - consumed).min(ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1);
        ZkAmsPhase23RnsLinkChunkCommitmentV1::new(
            family,
            chunk_index,
            u16::try_from(chunk_count).unwrap(),
            u32::try_from(logical_value_count).unwrap(),
            u32::try_from(used_slots).unwrap(),
            canonical_absent_chunk_bitmap_v1(u16::try_from(chunk_count).unwrap()).unwrap(),
            hyrax_point(0),
            commitment_digests(family, chunk_index),
        )
        .unwrap()
    }

    fn ordered_commitments() -> Vec<ZkAmsPhase23RnsLinkChunkCommitmentV1> {
        RNS_LINK_FAMILY_ORDER_V1
            .into_iter()
            .flat_map(|family| {
                let chunk_count =
                    expected_logical_values_v1(family).div_ceil(ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1);
                (0..chunk_count)
                    .map(move |index| chunk_commitment(family, u16::try_from(index).unwrap()))
            })
            .collect()
    }

    fn test_prechallenge() -> ZkAmsPhase23RnsLinkPrechallengeV1 {
        ZkAmsPhase23RnsLinkPrechallengeV1::from_ordered_commitments(
            test_context(b"network"),
            &ordered_commitments(),
        )
        .unwrap()
    }

    fn ordinary_product(a: &[u64], r: &[i64]) -> Vec<i64> {
        let mut product = vec![0_i64; a.len() + r.len() - 1];
        for (left_index, &left) in a.iter().enumerate() {
            for (right_index, &right) in r.iter().enumerate() {
                product[left_index + right_index] += i64::try_from(left).unwrap() * right;
            }
        }
        product
    }

    fn tiny_witness_with(mut a: Vec<u64>, mut message: Vec<u64>) -> TinyRnsLinkWitnessV1 {
        assert_eq!(a.len(), TINY_N);
        assert_eq!(message.len(), TINY_N);
        let r = vec![1, -1, 0, 1, 0, -1, 1, 0];
        let e = vec![-2, -1, 0, 1, 2, -2, 1, 0];
        // Keep ownership local and explicit even if a caller passes spare
        // capacity containing a prior test vector.
        a.shrink_to_fit();
        message.shrink_to_fit();
        let product = ordinary_product(&a, &r);
        let quotient: Vec<i64> = (0..TINY_N - 1)
            .map(|index| product[TINY_N + index])
            .collect();
        let mut ciphertext = Vec::with_capacity(TINY_N);
        let mut carries = Vec::with_capacity(TINY_N);
        for index in 0..TINY_N {
            let high = quotient.get(index).copied().unwrap_or(0);
            let reduced = product[index]
                + i64::try_from(TINY_P).unwrap() * e[index]
                + i64::try_from(message[index]).unwrap()
                - high;
            let residue = reduced.rem_euclid(i64::try_from(TINY_Q).unwrap());
            ciphertext.push(u64::try_from(residue).unwrap());
            carries.push((reduced - residue) / i64::try_from(TINY_Q).unwrap());
        }
        TinyRnsLinkWitnessV1 {
            a,
            r,
            e,
            message,
            ciphertext,
            quotient,
            carries,
            logical_slots: [0, 1, TINY_P - 1, 0],
            used_slots: 3,
            last_row: [4, 5, 0, 0],
            last_row_used: 2,
            absent_chunk_bitmap: 0b1110,
        }
    }

    fn honest_tiny_witness() -> TinyRnsLinkWitnessV1 {
        tiny_witness_with(
            vec![3, 5, 7, 11, 13, 17, 19, 23],
            vec![TINY_P - 1, 0, 1, 2, 3, 4, 5, 6],
        )
    }

    fn eval_i64(coefficients: &[i64], point: u64, modulus: u64) -> u64 {
        coefficients
            .iter()
            .rev()
            .fold(0_u64, |accumulator, coefficient| {
                let product = (u128::from(accumulator) * u128::from(point)) % u128::from(modulus);
                let coefficient =
                    u64::try_from(i128::from(*coefficient).rem_euclid(i128::from(modulus)))
                        .unwrap();
                u64::try_from((product + u128::from(coefficient)) % u128::from(modulus)).unwrap()
            })
    }

    fn eval_u64(coefficients: &[u64], point: u64, modulus: u64) -> u64 {
        coefficients
            .iter()
            .rev()
            .fold(0_u64, |accumulator, coefficient| {
                u64::try_from(
                    (u128::from(accumulator) * u128::from(point) + u128::from(*coefficient))
                        % u128::from(modulus),
                )
                .unwrap()
            })
    }

    fn mod_add(left: u64, right: u64, modulus: u64) -> u64 {
        u64::try_from((u128::from(left) + u128::from(right)) % u128::from(modulus)).unwrap()
    }

    fn mod_mul(left: u64, right: u64, modulus: u64) -> u64 {
        u64::try_from((u128::from(left) * u128::from(right)) % u128::from(modulus)).unwrap()
    }

    fn mod_sub(left: u64, right: u64, modulus: u64) -> u64 {
        if left >= right {
            left - right
        } else {
            modulus - (right - left)
        }
    }

    fn mod_pow(mut base: u64, mut exponent: usize, modulus: u64) -> u64 {
        let mut result = 1_u64;
        while exponent > 0 {
            if exponent & 1 == 1 {
                result = mod_mul(result, base, modulus);
            }
            base = mod_mul(base, base, modulus);
            exponent >>= 1;
        }
        result
    }

    fn verify_tiny_relation(
        witness: &TinyRnsLinkWitnessV1,
        points: &[u64],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if witness.a.len() != TINY_N
            || witness.r.as_slice().len() != TINY_N
            || witness.e.as_slice().len() != TINY_N
            || witness.message.len() != TINY_N
            || witness.ciphertext.len() != TINY_N
            || witness.quotient.as_slice().len() != TINY_N - 1
            || witness.carries.as_slice().len() != TINY_N
            || points.len() != RNS_LINK_EVALUATIONS_PER_LIMB_V1
            || points.iter().enumerate().any(|(index, point)| {
                *point == 0 || *point >= TINY_Q || points[..index].contains(point)
            })
            || witness.a.iter().any(|coefficient| *coefficient >= TINY_Q)
            || witness
                .message
                .iter()
                .any(|coefficient| *coefficient >= TINY_P)
            || witness
                .ciphertext
                .iter()
                .any(|coefficient| *coefficient >= TINY_Q)
            || witness
                .r
                .as_slice()
                .iter()
                .any(|coefficient| coefficient.unsigned_abs() > TINY_R_ABS_BOUND as u64)
            || witness
                .e
                .as_slice()
                .iter()
                .any(|coefficient| coefficient.unsigned_abs() > TINY_E_ABS_BOUND as u64)
            || witness
                .quotient
                .as_slice()
                .iter()
                .any(|coefficient| coefficient.unsigned_abs() > TINY_H_ABS_BOUND as u64)
            || witness
                .carries
                .as_slice()
                .iter()
                .any(|coefficient| coefficient.unsigned_abs() > TINY_CARRY_ABS_BOUND as u64)
            || witness.used_slots == 0
            || witness.used_slots > witness.logical_slots.len()
            || witness.logical_slots[..witness.used_slots]
                .iter()
                .any(|slot| *slot >= TINY_P)
            || witness.logical_slots[witness.used_slots..]
                .iter()
                .any(|slot| *slot != 0)
            || witness.last_row_used == 0
            || witness.last_row_used > witness.last_row.len()
            || witness.last_row[..witness.last_row_used]
                .iter()
                .any(|slot| *slot >= TINY_P)
            || witness.last_row[witness.last_row_used..]
                .iter()
                .any(|slot| *slot != 0)
            || witness.absent_chunk_bitmap != 0b1110
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }

        let product = ordinary_product(&witness.a, witness.r.as_slice());
        let q = i64::try_from(TINY_Q).unwrap();
        let p = i64::try_from(TINY_P).unwrap();
        for index in 0..TINY_N {
            let expected_high = product.get(TINY_N + index).copied().unwrap_or(0);
            if witness.quotient.as_slice().get(index).copied().unwrap_or(0) != expected_high {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
            let exact = product[index]
                + p * witness.e.as_slice()[index]
                + i64::try_from(witness.message[index]).unwrap()
                - i64::try_from(witness.ciphertext[index]).unwrap()
                - expected_high;
            let carry = witness.carries.as_slice()[index];
            if exact != q * carry || exact.rem_euclid(q) != 0 {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
        }

        for &point in points {
            let a = eval_u64(&witness.a, point, TINY_Q);
            let r = eval_i64(witness.r.as_slice(), point, TINY_Q);
            let e = eval_i64(witness.e.as_slice(), point, TINY_Q);
            let message = eval_u64(&witness.message, point, TINY_Q);
            let ciphertext = eval_u64(&witness.ciphertext, point, TINY_Q);
            let quotient = eval_i64(witness.quotient.as_slice(), point, TINY_Q);
            let left = mod_sub(
                mod_add(
                    mod_add(mod_mul(a, r, TINY_Q), mod_mul(TINY_P, e, TINY_Q), TINY_Q),
                    message,
                    TINY_Q,
                ),
                ciphertext,
                TINY_Q,
            );
            let x_n_plus_one = mod_add(mod_pow(point, TINY_N, TINY_Q), 1, TINY_Q);
            let right = mod_mul(x_n_plus_one, quotient, TINY_Q);
            if left != right {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
        }
        Ok(())
    }

    fn tiny_points() -> Vec<u64> {
        derive_evaluation_points_for_moduli_v1(&test_prechallenge(), &[TINY_Q])
            .unwrap()
            .points
            .into_iter()
            .map(|point| point.value)
            .collect()
    }

    #[test]
    fn release_points_are_deterministic_canonical_nonzero_and_distinct() {
        let prechallenge = test_prechallenge();
        let first = derive_release_evaluation_points_v1(&prechallenge).unwrap();
        let second = derive_release_evaluation_points_v1(&prechallenge).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.points.len(), release_profile_v1().moduli.len() * 5);
        for limb in first.points.chunks_exact(5) {
            assert_eq!(limb.len(), 5);
            for (index, point) in limb.iter().enumerate() {
                assert!(point.value > 0 && point.value < point.modulus);
                assert!(!limb[..index].iter().any(|prior| prior.value == point.value));
            }
        }
        first.validate_for_release(&prechallenge).unwrap();
    }

    #[test]
    fn mutable_readiness_and_kat_evidence_are_outside_the_proof_transcript() {
        let prechallenge = test_prechallenge();
        let baseline = derive_release_evaluation_points_v1(&prechallenge).unwrap();
        let context = test_context(b"network");
        assert_eq!(
            context.algorithm_manifest_digest,
            immutable_algorithm_manifest_digest_v1().unwrap()
        );

        let mut readiness = super::super::manifest::zk_ams_mkhe_readiness_digest_v1().unwrap();
        let mut release_kat = super::super::manifest::zk_ams_mkhe_release_manifest_v1()
            .unwrap()
            .release_kat_digest;
        let original_readiness = readiness;
        let original_kat = release_kat;
        readiness[0] ^= 1;
        release_kat[31] ^= 1;
        assert_ne!(readiness, original_readiness);
        assert_ne!(release_kat, original_kat);

        // Neither mutable value is accepted by the context or challenge API.
        // Installing measured evidence therefore cannot perturb a pinned proof.
        assert_eq!(
            baseline,
            derive_release_evaluation_points_v1(&prechallenge).unwrap()
        );
        assert_ne!(context.algorithm_manifest_digest, readiness);
        assert_ne!(context.algorithm_manifest_digest, release_kat);
    }

    #[test]
    fn production_challenge_source_has_no_readiness_or_kat_reference() {
        let source = include_str!("phase23_rns_link.rs");
        let production = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production source prefix");
        assert!(!production.contains("zk_ams_mkhe_readiness_digest_v1"));
        assert!(!production.contains("release_kat_digest"));
        assert!(!production.contains(".squeeze(b\""));
        assert!(!production.contains(".absorb_scalar(b\""));
        assert!(!production.contains(".absorb_univariate(b\""));
        for label in RNS_LINK_MANIFEST_TRANSCRIPT_LABELS_V1 {
            let label = core::str::from_utf8(label).expect("ASCII transcript label");
            let literal = format!("b\"{label}\"");
            assert_eq!(
                production.matches(&literal).count(),
                1,
                "transcript label must have one constant definition and no literal call-site copy: {label}"
            );
        }
        let _: RnsLinkContextConstructorV1 = RNS_LINK_CONTEXT_SIGNATURE_GUARD_V1;
        let _: RnsLinkChallengeConstructorV1 = RNS_LINK_CHALLENGE_SIGNATURE_GUARD_V1;
    }

    fn audited_scalars(values: &[u64]) -> AuditedRnsLinkSecretScalarsV1 {
        let mut scalars = AuditedRnsLinkSecretScalarsV1::with_capacity(values.len());
        for value in values {
            scalars.push(Scalar::from_u64(*value));
        }
        scalars
    }

    #[test]
    fn native_bitness_sumcheck_and_ipa_roundtrip() {
        let context = test_prechallenge().transcript_digest;
        let (statement, proof) = prove_rns_link_bitness_v1(
            context,
            audited_scalars(&[0, 1, 1, 0, 1, 0, 0, 1]),
            Scalar::from_u64(29),
        )
        .unwrap();
        assert_eq!(proof.sumcheck.rounds.len(), 3);
        assert_eq!(proof.ipa.left.len(), 4);
        assert_eq!(proof.ipa.right.len(), 4);
        verify_rns_link_bitness_v1(context, &statement, &proof).unwrap();
        assert_eq!(
            ZkAmsPhase23RnsLinkIpaKeyV1::derive(16).unwrap().digest,
            ZkAmsPhase23RnsLinkIpaKeyV1::derive(16).unwrap().digest
        );
    }

    #[test]
    fn native_bitness_sumcheck_and_ipa_reject_every_splice() {
        let context = test_prechallenge().transcript_digest;
        let (statement, proof) = prove_rns_link_bitness_v1(
            context,
            audited_scalars(&[0, 1, 1, 0, 1, 0, 0, 1]),
            Scalar::from_u64(31),
        )
        .unwrap();

        let mut wrong_context = context;
        wrong_context[0] ^= 1;
        assert_eq!(
            verify_rns_link_bitness_v1(wrong_context, &statement, &proof),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let mut changed_statement = statement.clone();
        changed_statement.key_digest[0] ^= 1;
        assert_eq!(
            verify_rns_link_bitness_v1(context, &changed_statement, &proof),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        let mut changed_statement = statement.clone();
        changed_statement.commitment =
            VegaT256PointV1::from_non_identity_wire_bytes_exact(&hyrax_point(1)).unwrap();
        assert_eq!(
            verify_rns_link_bitness_v1(context, &changed_statement, &proof),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let mut changed_sumcheck = proof.clone();
        changed_sumcheck.sumcheck.rounds[0].coefficients_except_linear[0] += Scalar::one();
        assert_eq!(
            verify_rns_link_bitness_v1(context, &statement, &changed_sumcheck),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        let mut changed_evaluation = proof.clone();
        changed_evaluation.evaluation += Scalar::one();
        assert_eq!(
            verify_rns_link_bitness_v1(context, &statement, &changed_evaluation),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        let mut truncated_ipa = proof.clone();
        truncated_ipa.ipa.left.pop();
        assert_eq!(
            verify_rns_link_bitness_v1(context, &statement, &truncated_ipa),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        let mut reordered_ipa = proof.clone();
        reordered_ipa.ipa.left.swap(0, 1);
        assert_eq!(
            verify_rns_link_bitness_v1(context, &statement, &reordered_ipa),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        let mut identity_ipa = proof.clone();
        identity_ipa.ipa.right[0] = VegaT256PointV1::identity();
        assert_eq!(
            verify_rns_link_bitness_v1(context, &statement, &identity_ipa),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        assert_eq!(
            prove_rns_link_bitness_v1(
                context,
                audited_scalars(&[0, 1, 2, 0, 1, 0, 0, 1]),
                Scalar::from_u64(37),
            ),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        assert_eq!(
            prove_rns_link_bitness_v1(context, audited_scalars(&[0, 1, 0]), Scalar::from_u64(41),),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
    }

    fn rns_link_expensive_path_counts_v1() -> (usize, usize) {
        let derivations = RNS_LINK_IPA_KEY_DERIVATIONS_V1.with(std::cell::Cell::get);
        let allocations = RNS_LINK_CODEC_BODY_ALLOCATIONS_V1.with(std::cell::Cell::get);
        (derivations, allocations)
    }

    fn assert_bitness_wire_preflight_rejects_v1(
        context: [u8; 32],
        statement: &ZkAmsPhase23RnsLinkBitnessStatementV1,
        bytes: &[u8],
        axis: &str,
    ) {
        let before = rns_link_expensive_path_counts_v1();
        assert_eq!(
            verify_rns_link_bitness_wire_v1(context, statement, bytes),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
            "malformed wire axis unexpectedly passed: {axis}"
        );
        assert_eq!(
            rns_link_expensive_path_counts_v1(),
            before,
            "malformed wire reached proof-vector allocation or IPA generator derivation: {axis}"
        );
    }

    #[test]
    fn native_bitness_codec_roundtrip_is_exact_and_bounded() {
        let context = test_prechallenge().transcript_digest;
        let (statement, proof) = prove_rns_link_bitness_v1(
            context,
            audited_scalars(&[0, 1, 1, 0, 1, 0, 0, 1]),
            Scalar::from_u64(43),
        )
        .unwrap();
        let bytes = encode_rns_link_bitness_proof_v1(&statement, &proof).unwrap();
        let shape = rns_link_bitness_codec_shape_v1(&statement).unwrap();
        assert_eq!(bytes.len(), shape.total_bytes);
        assert_eq!(bytes.len(), 728);
        assert!(bytes.len() <= RNS_LINK_BITNESS_CODEC_MAX_BYTES_V1);
        let decoded = decode_rns_link_bitness_proof_v1(&statement, &bytes).unwrap();
        assert_eq!(decoded, proof);
        assert_eq!(
            encode_rns_link_bitness_proof_v1(&statement, &decoded).unwrap(),
            bytes,
            "canonical decode must re-encode byte-identically"
        );
        verify_rns_link_bitness_wire_v1(context, &statement, &bytes).unwrap();

        let mut maximum_shape = statement;
        maximum_shape.value_count =
            u16::try_from(RNS_LINK_BITNESS_MAX_VALUES_V1).expect("max value count fits");
        let maximum_shape = rns_link_bitness_codec_shape_v1(&maximum_shape).unwrap();
        assert_eq!(
            maximum_shape.body_bytes,
            RNS_LINK_BITNESS_CODEC_MAX_BODY_BYTES_V1
        );
        assert_eq!(
            maximum_shape.total_bytes,
            RNS_LINK_BITNESS_CODEC_MAX_BYTES_V1
        );
        assert_eq!(maximum_shape.total_bytes, 1_862);
    }

    #[test]
    fn malformed_bitness_object_statements_reject_before_key_derivation() {
        let context = test_prechallenge().transcript_digest;
        let (statement, proof) = prove_rns_link_bitness_v1(
            context,
            audited_scalars(&[0, 1, 1, 0, 1, 0, 0, 1]),
            Scalar::from_u64(53),
        )
        .unwrap();
        let mut cases = Vec::new();
        for value_count in [0_u16, 1, 3, 2_048] {
            let mut changed = statement.clone();
            changed.value_count = value_count;
            cases.push(("invalid value count", changed));
        }
        let mut changed = statement.clone();
        changed.commitment = VegaT256PointV1::identity();
        cases.push(("identity commitment", changed));
        let mut changed = statement.clone();
        changed.relation_context_digest = [0; 32];
        cases.push(("zero relation-context digest", changed));
        let mut changed = statement.clone();
        changed.key_digest = [0; 32];
        cases.push(("zero key digest", changed));

        for (axis, changed) in cases {
            let before = RNS_LINK_IPA_KEY_DERIVATIONS_V1.with(std::cell::Cell::get);
            assert_eq!(
                verify_rns_link_bitness_v1(context, &changed, &proof),
                Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
                "malformed object statement passed: {axis}"
            );
            assert_eq!(
                RNS_LINK_IPA_KEY_DERIVATIONS_V1.with(std::cell::Cell::get),
                before,
                "malformed object statement derived IPA generators: {axis}"
            );
        }
    }

    #[test]
    fn native_bitness_codec_rejects_every_count_and_length_before_expensive_work() {
        let context = test_prechallenge().transcript_digest;
        let (statement, proof) = prove_rns_link_bitness_v1(
            context,
            audited_scalars(&[0, 1, 1, 0, 1, 0, 0, 1]),
            Scalar::from_u64(47),
        )
        .unwrap();
        let canonical = encode_rns_link_bitness_proof_v1(&statement, &proof).unwrap();

        let byte_axes = [
            (8, "version"),
            (9, "flags"),
            (
                BITNESS_CODEC_SUMCHECK_ROUNDS_OFFSET_V1,
                "sumcheck round count",
            ),
            (BITNESS_CODEC_SUMCHECK_DEGREE_OFFSET_V1, "sumcheck degree"),
            (BITNESS_CODEC_IPA_ROUNDS_OFFSET_V1, "IPA round count"),
            (BITNESS_CODEC_IPA_LEFT_COUNT_OFFSET_V1, "IPA left count"),
            (BITNESS_CODEC_IPA_RIGHT_COUNT_OFFSET_V1, "IPA right count"),
            (BITNESS_CODEC_POINT_BYTES_OFFSET_V1, "point width"),
            (BITNESS_CODEC_SCALAR_BYTES_OFFSET_V1, "scalar width"),
            (BITNESS_CODEC_RESERVED_OFFSET_V1, "reserved byte"),
        ];
        for (offset, axis) in byte_axes {
            let mut changed = canonical.clone();
            changed[offset] ^= 1;
            assert_bitness_wire_preflight_rejects_v1(context, &statement, &changed, axis);
        }

        let u16_axes = [
            (10, "header length"),
            (BITNESS_CODEC_VALUE_COUNT_OFFSET_V1, "value count"),
            (
                BITNESS_CODEC_SUMCHECK_COEFFICIENT_COUNT_OFFSET_V1,
                "sumcheck coefficient count",
            ),
        ];
        for (offset, axis) in u16_axes {
            let mut changed = canonical.clone();
            changed[offset + 1] ^= 1;
            assert_bitness_wire_preflight_rejects_v1(context, &statement, &changed, axis);
        }

        let length_axes = [
            (
                BITNESS_CODEC_SUMCHECK_BYTES_OFFSET_V1,
                "sumcheck byte length",
            ),
            (
                BITNESS_CODEC_EVALUATION_BYTES_OFFSET_V1,
                "evaluation byte length",
            ),
            (
                BITNESS_CODEC_IPA_LEFT_BYTES_OFFSET_V1,
                "IPA left byte length",
            ),
            (
                BITNESS_CODEC_IPA_RIGHT_BYTES_OFFSET_V1,
                "IPA right byte length",
            ),
            (
                BITNESS_CODEC_FINAL_WITNESS_BYTES_OFFSET_V1,
                "final-witness byte length",
            ),
            (BITNESS_CODEC_BODY_BYTES_OFFSET_V1, "body byte length"),
            (BITNESS_CODEC_TOTAL_BYTES_OFFSET_V1, "total byte length"),
        ];
        for (offset, axis) in length_axes {
            let mut changed = canonical.clone();
            changed[offset + 7] ^= 1;
            assert_bitness_wire_preflight_rejects_v1(context, &statement, &changed, axis);

            let mut overflow = canonical.clone();
            overflow[offset..offset + 8].fill(0xff);
            assert_bitness_wire_preflight_rejects_v1(
                context,
                &statement,
                &overflow,
                &format!("arithmetic-overflow {axis}"),
            );
        }

        let mut magic = canonical.clone();
        magic[0] ^= 1;
        assert_bitness_wire_preflight_rejects_v1(context, &statement, &magic, "magic");
        let mut manifest = canonical.clone();
        manifest[BITNESS_CODEC_MANIFEST_OFFSET_V1] ^= 1;
        assert_bitness_wire_preflight_rejects_v1(
            context,
            &statement,
            &manifest,
            "algorithm manifest digest",
        );

        let mut trailing = canonical.clone();
        trailing.push(0);
        assert_bitness_wire_preflight_rejects_v1(context, &statement, &trailing, "trailing byte");
        assert_bitness_wire_preflight_rejects_v1(
            context,
            &statement,
            &canonical[..canonical.len() - 1],
            "truncated body",
        );
        assert_bitness_wire_preflight_rejects_v1(
            context,
            &statement,
            &canonical[..RNS_LINK_BITNESS_CODEC_HEADER_BYTES_V1 - 1],
            "truncated header",
        );
        let mut oversized = canonical.clone();
        oversized.resize(RNS_LINK_BITNESS_CODEC_MAX_BYTES_V1 + 1, 0);
        assert_bitness_wire_preflight_rejects_v1(context, &statement, &oversized, "codec ceiling");

        let mut noncanonical_scalar = canonical.clone();
        noncanonical_scalar[RNS_LINK_BITNESS_CODEC_HEADER_BYTES_V1
            ..RNS_LINK_BITNESS_CODEC_HEADER_BYTES_V1 + RNS_LINK_SCALAR_WIRE_BYTES_V1]
            .fill(0xff);
        assert_bitness_wire_preflight_rejects_v1(
            context,
            &statement,
            &noncanonical_scalar,
            "noncanonical scalar",
        );
        let layout = preflight_rns_link_bitness_wire_v1(&statement, &canonical).unwrap();
        let mut identity_point = canonical;
        identity_point[layout.ipa_left_offset] = 0x40;
        identity_point
            [layout.ipa_left_offset + 1..layout.ipa_left_offset + RNS_LINK_POINT_WIRE_BYTES_V1]
            .fill(0);
        assert_bitness_wire_preflight_rejects_v1(
            context,
            &statement,
            &identity_point,
            "identity point",
        );
    }

    #[test]
    fn every_manifest_label_degree_dimension_and_count_axis_is_bound() {
        let canonical = canonical_algorithm_manifest_inputs_v1().unwrap();
        let baseline = immutable_algorithm_manifest_digest_from_inputs_v1(&canonical).unwrap();
        assert_eq!(baseline, immutable_algorithm_manifest_digest_v1().unwrap());

        let mut changed = canonical.clone();
        changed.version ^= 1;
        assert_ne!(
            immutable_algorithm_manifest_digest_from_inputs_v1(&changed).unwrap(),
            baseline
        );
        for digest_axis in 0..3 {
            let mut changed = canonical.clone();
            match digest_axis {
                0 => changed.profile_digest[0] ^= 1,
                1 => changed.map_set_digest[0] ^= 1,
                2 => changed.generator_basis_digest[0] ^= 1,
                _ => unreachable!(),
            }
            assert_ne!(
                immutable_algorithm_manifest_digest_from_inputs_v1(&changed).unwrap(),
                baseline,
                "immutable digest axis {digest_axis}"
            );
        }
        for index in 0..canonical.dimensions.len() {
            let mut changed = canonical.clone();
            changed.dimensions[index] ^= 1;
            assert_ne!(
                immutable_algorithm_manifest_digest_from_inputs_v1(&changed).unwrap(),
                baseline,
                "dimension/resource axis {index}"
            );
        }
        for index in 0..canonical.family_shapes.len() {
            let mut changed = canonical.clone();
            changed.family_shapes[index].0 ^= 0x80;
            assert_ne!(
                immutable_algorithm_manifest_digest_from_inputs_v1(&changed).unwrap(),
                baseline,
                "family tag axis {index}"
            );
            let mut changed = canonical.clone();
            changed.family_shapes[index].1 ^= 1;
            assert_ne!(
                immutable_algorithm_manifest_digest_from_inputs_v1(&changed).unwrap(),
                baseline,
                "family logical-count axis {index}"
            );
        }
        for index in 0..canonical.domains.len() {
            let mut changed = canonical.clone();
            changed.domains[index] = b"mutated-rns-link-domain";
            assert_ne!(
                immutable_algorithm_manifest_digest_from_inputs_v1(&changed).unwrap(),
                baseline,
                "domain axis {index}"
            );
        }
        for index in 0..canonical.transcript_labels.len() {
            let mut changed = canonical.clone();
            changed.transcript_labels[index] = b"mutated-rns-link-label";
            assert_ne!(
                immutable_algorithm_manifest_digest_from_inputs_v1(&changed).unwrap(),
                baseline,
                "transcript/generator label axis {index}"
            );
        }
        for index in 0..canonical.format_descriptors.len() {
            let mut changed = canonical.clone();
            changed.format_descriptors[index] = b"mutated-rns-link-descriptor";
            assert_ne!(
                immutable_algorithm_manifest_digest_from_inputs_v1(&changed).unwrap(),
                baseline,
                "relation/codec descriptor axis {index}"
            );
        }
        let mut changed = canonical;
        changed.codec_magic[0] ^= 1;
        assert_ne!(
            immutable_algorithm_manifest_digest_from_inputs_v1(&changed).unwrap(),
            baseline
        );
    }

    #[test]
    fn prechallenge_rejects_reorder_duplicate_omit_and_trailing_family() {
        let context = test_context(b"network");
        let baseline = ordered_commitments();

        let mut reordered = baseline.clone();
        reordered.swap(0, 1);
        assert_eq!(
            ZkAmsPhase23RnsLinkPrechallengeV1::from_ordered_commitments(context, &reordered),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let mut duplicate = baseline.clone();
        duplicate[1] = duplicate[0];
        assert_eq!(
            ZkAmsPhase23RnsLinkPrechallengeV1::from_ordered_commitments(context, &duplicate),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let mut omitted = baseline.clone();
        omitted.pop();
        assert_eq!(
            ZkAmsPhase23RnsLinkPrechallengeV1::from_ordered_commitments(context, &omitted),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let mut trailing = baseline;
        trailing.push(chunk_commitment(ZkAmsPhase23RnsLinkFamilyV1::RW, 0));
        assert_eq!(
            ZkAmsPhase23RnsLinkPrechallengeV1::from_ordered_commitments(context, &trailing),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
    }

    #[test]
    fn every_context_and_commitment_axis_precedes_and_changes_challenges() {
        let baseline_commitments = ordered_commitments();
        let baseline_context = test_context(b"network");
        let baseline_pre = ZkAmsPhase23RnsLinkPrechallengeV1::from_ordered_commitments(
            baseline_context,
            &baseline_commitments,
        )
        .unwrap();
        let baseline = derive_evaluation_points_for_moduli_v1(&baseline_pre, &[TINY_Q]).unwrap();

        let mut context_variants = Vec::new();
        let mut changed = baseline_context;
        changed.profile_digest[0] ^= 1;
        context_variants.push(changed);
        let mut changed = baseline_context;
        changed.algorithm_manifest_digest[0] ^= 1;
        context_variants.push(changed);
        let mut changed = baseline_context;
        changed.network_context_digest[0] ^= 1;
        context_variants.push(changed);
        let mut changed = baseline_context;
        changed.statement_context_digest[0] ^= 1;
        context_variants.push(changed);
        let mut changed = baseline_context;
        changed.transcript_digest[0] ^= 1;
        context_variants.push(changed);
        let mut changed = baseline_context;
        changed.batch_digest[0] ^= 1;
        context_variants.push(changed);
        let mut changed = baseline_context;
        changed.roster_digest[0] ^= 1;
        context_variants.push(changed);
        let mut changed = baseline_context;
        changed.direct_key_admission_digest[0] ^= 1;
        context_variants.push(changed);
        let mut changed = baseline_context;
        changed.canonical_map_set_digest[0] ^= 1;
        context_variants.push(changed);
        for changed_context in context_variants {
            let changed_pre = ZkAmsPhase23RnsLinkPrechallengeV1::from_ordered_commitments(
                changed_context,
                &baseline_commitments,
            )
            .unwrap();
            assert_ne!(
                baseline,
                derive_evaluation_points_for_moduli_v1(&changed_pre, &[TINY_Q]).unwrap()
            );
            assert_eq!(
                derive_release_evaluation_points_v1(&baseline_pre)
                    .unwrap()
                    .validate_for_release(&changed_pre),
                Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
            );
        }

        let mut commitment_variants = Vec::new();
        let mut changed = baseline_commitments.clone();
        for commitment in &mut changed[..8] {
            commitment.digests.layout_digest = digest(b"changed-x-layout");
        }
        commitment_variants.push(changed);
        for mutate in [
            |digests: &mut ZkAmsPhase23RnsLinkCommitmentDigestsV1| {
                digests.ciphertext_digest = digest(b"changed-ciphertext")
            },
            |digests: &mut ZkAmsPhase23RnsLinkCommitmentDigestsV1| {
                digests.bit_planes_digest = digest(b"changed-bit-planes")
            },
            |digests: &mut ZkAmsPhase23RnsLinkCommitmentDigestsV1| {
                digests.small_openings_digest = digest(b"changed-small-openings")
            },
            |digests: &mut ZkAmsPhase23RnsLinkCommitmentDigestsV1| {
                digests.packing_trace_digest = digest(b"changed-packing")
            },
            |digests: &mut ZkAmsPhase23RnsLinkCommitmentDigestsV1| {
                digests.radix_carry_digest = digest(b"changed-carry")
            },
            |digests: &mut ZkAmsPhase23RnsLinkCommitmentDigestsV1| {
                digests.negacyclic_quotient_digest = digest(b"changed-quotient")
            },
            |digests: &mut ZkAmsPhase23RnsLinkCommitmentDigestsV1| {
                digests.padding_digest = digest(b"changed-padding")
            },
        ] {
            let mut changed = baseline_commitments.clone();
            mutate(&mut changed[0].digests);
            commitment_variants.push(changed);
        }
        let mut changed = baseline_commitments.clone();
        changed[0].hyrax_commitment = hyrax_point(1);
        commitment_variants.push(changed);
        for changed_commitments in commitment_variants {
            let changed_pre = ZkAmsPhase23RnsLinkPrechallengeV1::from_ordered_commitments(
                baseline_context,
                &changed_commitments,
            )
            .unwrap();
            assert_ne!(
                baseline,
                derive_evaluation_points_for_moduli_v1(&changed_pre, &[TINY_Q]).unwrap()
            );
        }

        let mut axes = [
            digest(b"network"),
            digest(b"statement"),
            digest(b"transcript"),
            digest(b"batch"),
            digest(b"roster"),
            digest(b"direct-key"),
            zk_ams_phase23_release_map_set_digest_v1().unwrap(),
        ];
        for index in 0..axes.len() {
            let original = axes[index];
            axes[index] = [0; 32];
            assert_eq!(
                ZkAmsPhase23RnsLinkContextV1::new(
                    axes[0], axes[1], axes[2], axes[3], axes[4], axes[5], axes[6],
                ),
                Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
            );
            axes[index] = original;
        }
        axes[6] = digest(b"caller-nominated-map-set");
        assert_eq!(
            ZkAmsPhase23RnsLinkContextV1::new(
                axes[0], axes[1], axes[2], axes[3], axes[4], axes[5], axes[6],
            ),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
    }

    #[test]
    fn commitment_shape_identity_and_absent_chunk_metadata_fail_closed() {
        let family = ZkAmsPhase23RnsLinkFamilyV1::X;
        let valid_digests = commitment_digests(family, 0);
        let logical_value_count = u32::try_from(RNS_LINK_X_LOGICAL_VALUES_V1).unwrap();
        let used_slots = u32::try_from(ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1).unwrap();
        let absent = canonical_absent_chunk_bitmap_v1(8).unwrap();
        assert_eq!(
            ZkAmsPhase23RnsLinkChunkCommitmentV1::new(
                family,
                0,
                8,
                logical_value_count,
                used_slots,
                absent,
                [0; 33],
                valid_digests,
            ),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        assert_eq!(
            ZkAmsPhase23RnsLinkChunkCommitmentV1::new(
                family,
                0,
                8,
                logical_value_count,
                used_slots,
                0,
                hyrax_point(0),
                valid_digests,
            ),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        assert_eq!(
            ZkAmsPhase23RnsLinkChunkCommitmentV1::new(
                family,
                0,
                8,
                logical_value_count,
                used_slots - 1,
                absent,
                hyrax_point(0),
                valid_digests,
            ),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        let mut zero_digest = valid_digests;
        zero_digest.padding_digest = [0; 32];
        assert_eq!(
            ZkAmsPhase23RnsLinkChunkCommitmentV1::new(
                family,
                0,
                8,
                logical_value_count,
                used_slots,
                absent,
                hyrax_point(0),
                zero_digest,
            ),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
    }

    #[test]
    fn rejection_sampler_is_unbiased_and_has_exact_retry_ceiling() {
        let threshold = TINY_Q.wrapping_neg() % TINY_Q;
        let accepted = threshold + ((5 + TINY_Q - (threshold % TINY_Q)) % TINY_Q);
        let mut calls = 0_usize;
        let value = sample_canonical_nonzero_distinct_v1(TINY_Q, &[], |_| {
            calls += 1;
            if calls == 1 { threshold - 1 } else { accepted }
        })
        .unwrap();
        assert_eq!(value, 5);
        assert_eq!(calls, 2);

        let mut zero_calls = 0_usize;
        assert_eq!(
            sample_canonical_nonzero_distinct_v1(TINY_Q, &[], |_| {
                zero_calls += 1;
                0
            }),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        assert_eq!(zero_calls, RNS_LINK_REJECTION_ATTEMPTS_V1);

        let mut duplicate_calls = 0_usize;
        assert_eq!(
            sample_canonical_nonzero_distinct_v1(TINY_Q, &[5], |_| {
                duplicate_calls += 1;
                accepted
            }),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        assert_eq!(duplicate_calls, RNS_LINK_REJECTION_ATTEMPTS_V1);
    }

    #[test]
    fn tiny_exact_integer_and_arbitrary_point_relation_accepts() {
        let points = tiny_points();
        assert_eq!(points.len(), 5);
        verify_tiny_relation(&honest_tiny_witness(), &points).unwrap();

        let boundary = tiny_witness_with(
            vec![TINY_Q - 1, 5, 7, 11, 13, 17, 19, 23],
            vec![TINY_P - 1, 0, 1, 2, 3, 4, 5, 6],
        );
        verify_tiny_relation(&boundary, &points).unwrap();
    }

    #[test]
    fn tiny_relation_rejects_boundaries_mutations_and_wrap_attempts() {
        let points = tiny_points();

        let mut q_boundary = honest_tiny_witness();
        q_boundary.a[0] = TINY_Q;
        assert_eq!(
            verify_tiny_relation(&q_boundary, &points),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let mut p_boundary = honest_tiny_witness();
        p_boundary.message[0] = TINY_P;
        assert_eq!(
            verify_tiny_relation(&p_boundary, &points),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let mut noncanonical_ciphertext = honest_tiny_witness();
        noncanonical_ciphertext.ciphertext[0] = TINY_Q;
        assert_eq!(
            verify_tiny_relation(&noncanonical_ciphertext, &points),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let mut bad_r = honest_tiny_witness();
        bad_r.r.as_mut_slice()[0] = TINY_R_ABS_BOUND + 1;
        assert_eq!(
            verify_tiny_relation(&bad_r, &points),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let mut bad_e = honest_tiny_witness();
        bad_e.e.as_mut_slice()[0] = TINY_E_ABS_BOUND + 1;
        assert_eq!(
            verify_tiny_relation(&bad_e, &points),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        for delta in [-1_i64, 1] {
            let mut bad_quotient = honest_tiny_witness();
            bad_quotient.quotient.as_mut_slice()[0] += delta;
            assert_eq!(
                verify_tiny_relation(&bad_quotient, &points),
                Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
            );

            let mut bad_carry = honest_tiny_witness();
            bad_carry.carries.as_mut_slice()[0] += delta;
            assert_eq!(
                verify_tiny_relation(&bad_carry, &points),
                Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
            );
        }

        let mut zero_point = points.clone();
        zero_point[0] = 0;
        assert_eq!(
            verify_tiny_relation(&honest_tiny_witness(), &zero_point),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        let mut duplicate_point = points;
        duplicate_point[1] = duplicate_point[0];
        assert_eq!(
            verify_tiny_relation(&honest_tiny_witness(), &duplicate_point),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
    }

    #[test]
    fn tiny_relation_rejects_every_padding_namespace() {
        let points = tiny_points();

        let mut logical_padding = honest_tiny_witness();
        logical_padding.logical_slots[3] = 1;
        assert_eq!(
            verify_tiny_relation(&logical_padding, &points),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let mut last_row_padding = honest_tiny_witness();
        last_row_padding.last_row[2] = 1;
        assert_eq!(
            verify_tiny_relation(&last_row_padding, &points),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let mut absent_chunk = honest_tiny_witness();
        absent_chunk.absent_chunk_bitmap ^= 1 << 2;
        assert_eq!(
            verify_tiny_relation(&absent_chunk, &points),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
    }

    #[test]
    fn secret_owners_redact_and_zeroize_success_error_and_unwind() {
        let before_success = zeroizing_t256_scalar_vec_drop_count_v1();
        {
            let secrets = ZkAmsPhase23RnsLinkWitnessSecretsV1::test_fixture();
            let debug = format!("{secrets:?}");
            assert!(debug.contains("[REDACTED]"));
            assert!(!debug.contains('7'));
            assert!(!debug.contains('9'));
        }
        assert_eq!(
            zeroizing_t256_scalar_vec_drop_count_v1(),
            before_success + 6
        );

        fn fail_after_taking_secrets(
            _secrets: ZkAmsPhase23RnsLinkWitnessSecretsV1,
        ) -> Result<(), ZkAmsMkheErrorV1> {
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        }
        let before_error = zeroizing_t256_scalar_vec_drop_count_v1();
        assert_eq!(
            fail_after_taking_secrets(ZkAmsPhase23RnsLinkWitnessSecretsV1::test_fixture()),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before_error + 6);

        let before_unwind = zeroizing_t256_scalar_vec_drop_count_v1();
        let unwind_result = catch_unwind(AssertUnwindSafe(|| {
            let _secrets = ZkAmsPhase23RnsLinkWitnessSecretsV1::test_fixture();
            panic!("intentional erasure audit");
        }));
        assert!(unwind_result.is_err());
        assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before_unwind + 6);
    }
}
