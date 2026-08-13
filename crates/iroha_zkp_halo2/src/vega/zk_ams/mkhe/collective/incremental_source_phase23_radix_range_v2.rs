//! Static Phase-23 radix/range prerequisite.
//!
//! This private child freezes the scalable base-2^15 topology, formulas,
//! accounting, and transcript order.  It does not construct commitments,
//! evaluate a relation, mint a proof, or grant authority.  Its four production
//! seals are uninhabited, and every gate that would claim actual algebra,
//! packing, zero knowledge, qPCS, operational qualification, or release stays
//! false.  The cross-basis masking entry is only a statistical-HVZK plan: its
//! 64-byte modular-reduction sampler is recorded at less than 2^-245 distance
//! from the ideal uniform-vector simulator, not as perfect zero knowledge.

#![allow(
    dead_code,
    reason = "static planning evidence is production-uninhabited"
)]
use core::convert::Infallible;
#[cfg(test)]
use core::sync::atomic::{AtomicUsize, Ordering};
use crate::vega::{VEGA_T256_SCALAR_MODULUS_BE_V1, sponge::Keccak256};
use super::{
    PHASE23_RECORD_COUNT_V1, PHASE23_RING_DEGREE_V1, ZkAmsMkheErrorV1, phase23_record_position_v1,
};
const RADIX_RANGE_VERSION_V2: u8 = 2;
const RADIX_BASE_V2: usize = 1 << 15;
const RADIX_LOW_LIMBS_V2: usize = 17;
const RADIX_TOP_BITS_PER_COEFFICIENT_V2: usize = 2;
const RADIX_LOW_DIGITS_PER_COEFFICIENT_V2: usize = 2 * RADIX_LOW_LIMBS_V2;
const RADIX_RECORDS_V2: usize = 43;
const RADIX_GROUPS_PER_RECORD_V2: usize = 8;
const RADIX_GROUPS_V2: usize = RADIX_RECORDS_V2 * RADIX_GROUPS_PER_RECORD_V2;
const RADIX_COEFFICIENTS_PER_GROUP_V2: usize = 16_384;
const RADIX_SOURCE_BLOCKS_PER_GROUP_V2: usize = 64;
const RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2: usize = 256;
const RADIX_COEFFICIENTS_V2: usize = RADIX_RECORDS_V2 * PHASE23_RING_DEGREE_V1;
const RADIX_LOW_DIGITS_V2: usize = RADIX_COEFFICIENTS_V2 * RADIX_LOW_DIGITS_PER_COEFFICIENT_V2;
const RADIX_TOP_BITS_V2: usize = RADIX_COEFFICIENTS_V2 * RADIX_TOP_BITS_PER_COEFFICIENT_V2;
const RADIX_INVERSE_PLANES_PER_GROUP_V2: usize = 2;
const RADIX_INVERSE_POINTS_PER_PLANE_V2: usize = RADIX_LOW_LIMBS_V2;
const RADIX_INVERSE_POINTS_PER_GROUP_V2: usize =
    RADIX_INVERSE_PLANES_PER_GROUP_V2 * RADIX_INVERSE_POINTS_PER_PLANE_V2;
const RADIX_COMMITMENT_POINTS_PER_GROUP_V2: usize = 2 * (2 * RADIX_LOW_LIMBS_V2 + 1);
const RADIX_RANGE_COMMITMENT_POINTS_V2: usize =
    RADIX_GROUPS_V2 * RADIX_COMMITMENT_POINTS_PER_GROUP_V2;
const RADIX_SOURCE_COEFFICIENT_COMMITMENT_POINTS_V2: usize = RADIX_GROUPS_V2;
const RADIX_RECORD_ORDER_V2: &[u8] = b"X1/U16/E16/RE1/W8/RW1";
const DECOMPOSITION_D_FORMULA_V2: &[u8] = b"D=sum_{h=0}^{16}(2^15)^h*d_h+(2^15)^17*b_d";
const DECOMPOSITION_S_FORMULA_V2: &[u8] = b"S=sum_{h=0}^{16}(2^15)^h*s_h+(2^15)^17*b_s";
const CANONICAL_VALUE_FORMULA_V2: &[u8] = b"v=D mod p;D+S=p-1";
const DIGIT_TABLE_FORMULA_V2: &[u8] = b"d_h,s_h in [0,32767]";
const TOP_BIT_FORMULA_V2: &[u8] = b"b_d*(b_d-1)=b_s*(b_s-1)=b_d*b_s=0";
const LOOKUP_FORMULA_V2: &[u8] =
    b"reject until z notin [0,32767];then U_D,U_S=(z-A)^-1;absorb Dinv then Sinv";
const SOURCE_COORDINATE_FORMULA_V2: &[u8] = b"source=(((record*8+group)*64+block)*256)+coefficient";
const PACKING_TRANSPOSE_FORMULA_V2: &[u8] =
    b"packing=((record*8+group)*16384)+(coefficient*64)+block";
const COMMITMENT_TOPOLOGY_FORMULA_V2: &[u8] =
    b"per-group:source1,D17,S17,Dinv17,Sinv17,Dtop1,Stop1";
const LOOKUP_SOUNDNESS_FORMULA_V2: &[u8] = b"191679039/(p-32768)<2^-228.48";
const CROSS_BASIS_HVZK_FORMULA_V2: &[u8] =
    b"64-byte-modular-reduction-vector-mask:distance-from-ideal<2^-245";
const STATIC_EVIDENCE_FORMULA_V2: &[u8] = b"planning-only:no-proof:no-authority:no-RSS:no-release";
const RADIX_WIRE_HEADER_BYTES_V2: usize = 1_024;
const RADIX_WIRE_TERMINAL_BP_BYTES_V2: usize = 50_688;
const RADIX_WIRE_CROSS_SCHNORR_BYTES_V2: usize = 32_866;
const RADIX_WIRE_SOURCE_COEFFICIENT_POINTS_BYTES_V2: usize = 11_352;
const RADIX_WIRE_DIGIT_SLACK_INVERSE_TOP_POINTS_BYTES_V2: usize = 794_640;
const RADIX_WIRE_MULTIPLICITY_BYTES_V2: usize = 33;
const RADIX_WIRE_CUBIC_MESSAGES_BYTES_V2: usize = 22_368;
const RADIX_CUBIC_MESSAGES_V2: usize = 233;
const RADIX_WIRE_HIDDEN_EVALUATION_COMMITMENTS_BYTES_V2: usize = 1_716;
const RADIX_HIDDEN_EVALUATION_COMMITMENTS_V2: usize = 52;
const RADIX_WIRE_COEFFICIENT_BASIS_IPAS_BYTES_V2: usize = 16_352;
const RADIX_COEFFICIENT_BASIS_IPAS_V2: usize = 16;
const RADIX_WIRE_TABLE_IPA_BYTES_V2: usize = 1_088;
const RADIX_WIRE_MASK_COMMITMENT_IPA_BYTES_V2: usize = 725;
const RADIX_WIRE_32_GATE_BP_BYTES_V2: usize = 834;
const RADIX_WIRE_PACKING_OPENINGS_BYTES_V2: usize = 1_216_031;
const RADIX_RANGE_WIRE_BYTES_V2: usize = 2_149_717;
const Q_PCS_WIRE_BYTES_V2: usize = 29_245_792;
const RADIX_Q_PCS_COMBINED_WIRE_BYTES_V2: usize = 31_395_509;
const RADIX_Q_PCS_COMBINED_CAP_BYTES_V2: usize = 32 * 1_048_576;
const RADIX_Q_PCS_COMBINED_MARGIN_BYTES_V2: usize = 2_158_923;
const RADIX_LOOKUP_SOUNDNESS_NUMERATOR_V2: u64 = 191_679_039;
const RADIX_LOOKUP_SOUNDNESS_BITS_X100_FLOOR_V2: u32 = 22_848;
const CROSS_BASIS_STATISTICAL_HVZK_BITS_V2: u16 = 245;
const RADIX_DIGIT_SLACK_EMISSIONS_V2: u64 = 191_627_264;
const RADIX_BATCH_INVERSIONS_MAX_V2: u64 = 5_848;
const RADIX_INVERSE_PASS_MULTIPLICATIONS_V2: u64 = 574_881_792;
const RADIX_FIXED_BASE_SOURCE_RANGE_TERMS_V2: u64 = 400_187_240;
const RADIX_FIXED_BASE_TERMINAL_TERMS_V2: u64 = 1_574_400;
const RADIX_SUMCHECK_VISITS_V2: u64 = 789_053_396;
const RADIX_PACKING_TRANSPOSE_STAGES_V2: u64 = 95_813_632;
const RADIX_COMMITTED_IPAS_V2: usize = 1_536;
const RADIX_COMMITTED_IPA_VECTOR_LENGTH_V2: usize = 2_048;
const RADIX_TABLE_IPAS_V2: usize = 1;
const RADIX_TABLE_IPA_VECTOR_LENGTH_V2: usize = 32_768;
const RADIX_EXTERNAL_IO_BYTES_V2: u64 = 26_846_528_789;
const RADIX_CONFIDENTIAL_SCRATCH_BYTES_V2: u64 = 6_836_977_664;
const RADIX_SOURCE_PUBLICATION_BYTES_V2: u64 = 7_152_600_416;
const Q_PCS_EXTERNAL_PEAK_BYTES_V2: u64 = 10_504_241_168;
const RADIX_LOCAL_HEAP_BYTES_V2: usize = 20_598_361;
const RADIX_RETAINED_SOURCE_ROOT_BYTES_V2: usize = 83_503_936;
const RADIX_PHASE_NAMED_HEAP_BYTES_V2: usize = 104_102_297;
const Q_PCS_CONSERVATIVE_HEAP_BYTES_V2: usize = 120_129_088;
const RADIX_Q_PCS_OVERLAP_HEAP_BYTES_V2: usize = 140_727_449;
const RADIX_Q_PCS_HEAP_CEILING_BYTES_V2: usize = 160 * 1_048_576;
const RADIX_Q_PCS_HEAP_MARGIN_BYTES_V2: usize = 27_044_711;
const SOURCE_ALGEBRA_ACTUALLY_VERIFIED_V2: bool = false;
const RADIX_DECOMPOSITION_ACTUALLY_VERIFIED_V2: bool = false;
const RADIX_CANONICAL_RANGE_ACTUALLY_VERIFIED_V2: bool = false;
const RADIX_LOOKUP_ACTUALLY_VERIFIED_V2: bool = false;
const ZERO_KNOWLEDGE_ACTUALLY_VERIFIED_V2: bool = false;
const PACKING_TRANSPOSE_ACTUALLY_VERIFIED_V2: bool = false;
const PACKING_EQUALITY_ACTUALLY_VERIFIED_V2: bool = false;
const HYRAX_ACTUALLY_VERIFIED_V2: bool = false;
const Q_PCS_REPLAY_ACTUALLY_VERIFIED_V2: bool = false;
const Q_PCS_HANDOFF_ACTUALLY_COMPLETE_V2: bool = false;
const OPERATIONAL_QUALIFICATION_ACCEPTED_V2: bool = false;
const RECEIPT_ACCEPTED_V2: bool = false;
const RSS_QUALIFIED_V2: bool = false;
const PROOF_MINTED_V2: bool = false;
const AUTHORITY_MINTED_V2: bool = false;
const RELEASE_COMPLETE_V2: bool = false;
const _: () = {
    assert!(RADIX_RECORDS_V2 == PHASE23_RECORD_COUNT_V1);
    assert!(RADIX_COEFFICIENTS_V2 == 5_636_096);
    assert!(RADIX_GROUPS_V2 == 344);
    assert!(RADIX_COEFFICIENTS_PER_GROUP_V2 == 64 * 256);
    assert!(RADIX_GROUPS_PER_RECORD_V2 * RADIX_COEFFICIENTS_PER_GROUP_V2 == PHASE23_RING_DEGREE_V1);
    assert!(RADIX_LOW_DIGITS_V2 == 191_627_264);
    assert!(RADIX_TOP_BITS_V2 == 11_272_192);
    assert!(RADIX_INVERSE_POINTS_PER_GROUP_V2 == 34);
    assert!(RADIX_COMMITMENT_POINTS_PER_GROUP_V2 == 70);
    assert!(RADIX_RANGE_COMMITMENT_POINTS_V2 == 24_080);
    assert!(RADIX_SOURCE_COEFFICIENT_COMMITMENT_POINTS_V2 == 344);
    assert!(RADIX_WIRE_SOURCE_COEFFICIENT_POINTS_BYTES_V2 == 344 * 33);
    assert!(RADIX_WIRE_DIGIT_SLACK_INVERSE_TOP_POINTS_BYTES_V2 == 24_080 * 33);
    assert!(RADIX_WIRE_CUBIC_MESSAGES_BYTES_V2 == RADIX_CUBIC_MESSAGES_V2 * 96);
    assert!(RADIX_WIRE_HIDDEN_EVALUATION_COMMITMENTS_BYTES_V2 == 52 * 33);
    assert!(
        RADIX_RANGE_WIRE_BYTES_V2
            == RADIX_WIRE_HEADER_BYTES_V2
                + RADIX_WIRE_TERMINAL_BP_BYTES_V2
                + RADIX_WIRE_CROSS_SCHNORR_BYTES_V2
                + RADIX_WIRE_SOURCE_COEFFICIENT_POINTS_BYTES_V2
                + RADIX_WIRE_DIGIT_SLACK_INVERSE_TOP_POINTS_BYTES_V2
                + RADIX_WIRE_MULTIPLICITY_BYTES_V2
                + RADIX_WIRE_CUBIC_MESSAGES_BYTES_V2
                + RADIX_WIRE_HIDDEN_EVALUATION_COMMITMENTS_BYTES_V2
                + RADIX_WIRE_COEFFICIENT_BASIS_IPAS_BYTES_V2
                + RADIX_WIRE_TABLE_IPA_BYTES_V2
                + RADIX_WIRE_MASK_COMMITMENT_IPA_BYTES_V2
                + RADIX_WIRE_32_GATE_BP_BYTES_V2
                + RADIX_WIRE_PACKING_OPENINGS_BYTES_V2
    );
    assert!(RADIX_Q_PCS_COMBINED_WIRE_BYTES_V2 == RADIX_RANGE_WIRE_BYTES_V2 + Q_PCS_WIRE_BYTES_V2);
    assert!(
        RADIX_Q_PCS_COMBINED_MARGIN_BYTES_V2
            == RADIX_Q_PCS_COMBINED_CAP_BYTES_V2 - RADIX_Q_PCS_COMBINED_WIRE_BYTES_V2
    );
    assert!(RADIX_DIGIT_SLACK_EMISSIONS_V2 == RADIX_LOW_DIGITS_V2 as u64);
    assert!(RADIX_BATCH_INVERSIONS_MAX_V2 == (RADIX_GROUPS_V2 * RADIX_LOW_LIMBS_V2) as u64);
    assert!(RADIX_INVERSE_PASS_MULTIPLICATIONS_V2 == RADIX_LOW_DIGITS_V2 as u64 * 3);
    assert!(RADIX_FIXED_BASE_SOURCE_RANGE_TERMS_V2 == RADIX_COEFFICIENTS_V2 as u64 * 71 + 24_424);
    assert!(RADIX_FIXED_BASE_TERMINAL_TERMS_V2 == 1_536 * 1_025);
    assert!(RADIX_SUMCHECK_VISITS_V2 == RADIX_COEFFICIENTS_V2 as u64 * 140 - 44);
    assert!(RADIX_PACKING_TRANSPOSE_STAGES_V2 == RADIX_COEFFICIENTS_V2 as u64 * 17);
    assert!(
        RADIX_PHASE_NAMED_HEAP_BYTES_V2
            == RADIX_LOCAL_HEAP_BYTES_V2 + RADIX_RETAINED_SOURCE_ROOT_BYTES_V2
    );
    assert!(
        RADIX_Q_PCS_OVERLAP_HEAP_BYTES_V2
            == Q_PCS_CONSERVATIVE_HEAP_BYTES_V2 + RADIX_LOCAL_HEAP_BYTES_V2
    );
    assert!(
        RADIX_Q_PCS_HEAP_MARGIN_BYTES_V2
            == RADIX_Q_PCS_HEAP_CEILING_BYTES_V2 - RADIX_Q_PCS_OVERLAP_HEAP_BYTES_V2
    );
    assert!(!SOURCE_ALGEBRA_ACTUALLY_VERIFIED_V2);
    assert!(!RADIX_DECOMPOSITION_ACTUALLY_VERIFIED_V2);
    assert!(!RADIX_CANONICAL_RANGE_ACTUALLY_VERIFIED_V2);
    assert!(!RADIX_LOOKUP_ACTUALLY_VERIFIED_V2);
    assert!(!ZERO_KNOWLEDGE_ACTUALLY_VERIFIED_V2);
    assert!(!PACKING_TRANSPOSE_ACTUALLY_VERIFIED_V2);
    assert!(!PACKING_EQUALITY_ACTUALLY_VERIFIED_V2);
    assert!(!HYRAX_ACTUALLY_VERIFIED_V2);
    assert!(!Q_PCS_REPLAY_ACTUALLY_VERIFIED_V2);
    assert!(!Q_PCS_HANDOFF_ACTUALLY_COMPLETE_V2);
    assert!(!OPERATIONAL_QUALIFICATION_ACCEPTED_V2);
    assert!(!RECEIPT_ACCEPTED_V2);
    assert!(!RSS_QUALIFIED_V2);
    assert!(!PROOF_MINTED_V2);
    assert!(!AUTHORITY_MINTED_V2);
    assert!(!RELEASE_COMPLETE_V2);
};
struct RadixRangeFamilyV2 {
    tag: u8,
    records: u8,
    label: &'static [u8],
}
const RADIX_RANGE_FAMILIES_V2: [RadixRangeFamilyV2; 6] = [
    RadixRangeFamilyV2 {
        tag: 1,
        records: 1,
        label: b"X",
    },
    RadixRangeFamilyV2 {
        tag: 2,
        records: 16,
        label: b"U",
    },
    RadixRangeFamilyV2 {
        tag: 3,
        records: 16,
        label: b"E",
    },
    RadixRangeFamilyV2 {
        tag: 4,
        records: 1,
        label: b"RE",
    },
    RadixRangeFamilyV2 {
        tag: 5,
        records: 8,
        label: b"W",
    },
    RadixRangeFamilyV2 {
        tag: 6,
        records: 1,
        label: b"RW",
    },
];
struct RadixRangeSourceCoordinateV2 {
    ordinal: u16,
    family: u8,
    group: u8,
    source_block: u8,
    coefficient: u16,
    source_index: u32,
    packing_index: u32,
}
fn source_coordinate_v2(
    ordinal: u16,
    group: usize,
    source_block: usize,
    coefficient: usize,
) -> Result<RadixRangeSourceCoordinateV2, ZkAmsMkheErrorV1> {
    if group >= RADIX_GROUPS_PER_RECORD_V2
        || source_block >= RADIX_SOURCE_BLOCKS_PER_GROUP_V2
        || coefficient >= RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let position = phase23_record_position_v1(ordinal)?;
    let group_base = (usize::from(ordinal) * RADIX_GROUPS_PER_RECORD_V2 + group)
        * RADIX_COEFFICIENTS_PER_GROUP_V2;
    let source_index =
        group_base + source_block * RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2 + coefficient;
    let packing_index = group_base + coefficient * RADIX_SOURCE_BLOCKS_PER_GROUP_V2 + source_block;
    Ok(RadixRangeSourceCoordinateV2 {
        ordinal,
        family: position.family as u8,
        group: u8::try_from(group).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        source_block: u8::try_from(source_block)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        coefficient: u16::try_from(coefficient)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        source_index: u32::try_from(source_index)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        packing_index: u32::try_from(packing_index)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    })
}
const RADIX_RANGE_TRANSCRIPT_FRAMES_V2: [&[u8]; 32] = [
    b"axes",
    b"ordered-manifests",
    b"source-and-output-receipts",
    b"qpcs-context",
    b"terminal-commitments",
    b"group-source-coefficient-commitments-344",
    b"d-low-digit-commitments",
    b"s-low-digit-commitments",
    b"d-top-bit-commitments",
    b"s-top-bit-commitments",
    b"lookup-multiplicity-commitment",
    b"zero-sum-mask-commitment",
    b"eta",
    b"cross-schnorr-nonce",
    b"cross-schnorr-challenge",
    b"cross-schnorr-response",
    b"packing-zeta",
    b"packing-eta-p",
    b"packing-rho",
    b"lookup-z-outside-digit-table",
    b"d-inverse-commitments",
    b"s-inverse-commitments",
    b"shard-challenges",
    b"constraint-challenges",
    b"hidden-evaluation-commitments",
    b"ipa-batching-challenges",
    b"gate32-relation",
    b"binding-digest",
    b"future-q-l-linkage",
    b"gamma",
    b"beta",
    b"qpcs",
];
const RADIX_RANGE_TOPOLOGY_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.radix-range.topology";
const RADIX_RANGE_FORMULA_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.radix-range.formulas";
const RADIX_RANGE_ACCOUNTING_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.radix-range.static-accounting";
const RADIX_RANGE_TRANSCRIPT_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.radix-range.transcript-manifest";
const RADIX_RANGE_PREREQUISITE_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.radix-range.static-prerequisite";
fn nonzero_digest_v2(digest: [u8; 32]) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(digest)
}
fn topology_digest_for_record_order_v2(
    record_order: &[u16; RADIX_RECORDS_V2],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut seen = [false; RADIX_RECORDS_V2];
    let mut hash = Keccak256::new();
    hash.update(RADIX_RANGE_TOPOLOGY_DOMAIN_V2);
    hash.update(&[RADIX_RANGE_VERSION_V2]);
    hash.update(RADIX_RECORD_ORDER_V2);
    hash.update(&(RADIX_RECORDS_V2 as u16).to_be_bytes());
    hash.update(&(RADIX_GROUPS_V2 as u16).to_be_bytes());
    hash.update(&(RADIX_COEFFICIENTS_V2 as u32).to_be_bytes());
    hash.update(&(RADIX_LOW_DIGITS_V2 as u32).to_be_bytes());
    for family in &RADIX_RANGE_FAMILIES_V2 {
        hash.update(&[family.tag, family.records, family.label.len() as u8]);
        hash.update(family.label);
    }
    for ordinal in record_order {
        let index = usize::from(*ordinal);
        if index >= seen.len() || seen[index] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        seen[index] = true;
        let position = phase23_record_position_v1(*ordinal)?;
        hash.update(&ordinal.to_be_bytes());
        hash.update(&[position.family as u8]);
        hash.update(&position.chunk_index.to_be_bytes());
        hash.update(&position.family_chunk_count.to_be_bytes());
        hash.update(&position.logical_value_count.to_be_bytes());
    }
    if seen.iter().any(|present| !present) {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    for formula in [
        SOURCE_COORDINATE_FORMULA_V2,
        PACKING_TRANSPOSE_FORMULA_V2,
        COMMITMENT_TOPOLOGY_FORMULA_V2,
    ] {
        hash.update(&(formula.len() as u16).to_be_bytes());
        hash.update(formula);
    }
    nonzero_digest_v2(hash.finalize())
}
fn exact_topology_digest_v2() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut order = [0_u16; RADIX_RECORDS_V2];
    for (ordinal, destination) in order.iter_mut().enumerate() {
        *destination =
            u16::try_from(ordinal).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    }
    topology_digest_for_record_order_v2(&order)
}
fn exact_formula_digest_v2() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(RADIX_RANGE_FORMULA_DOMAIN_V2);
    hash.update(&[RADIX_RANGE_VERSION_V2]);
    hash.update(&(RADIX_BASE_V2 as u32).to_be_bytes());
    hash.update(&(RADIX_LOW_LIMBS_V2 as u16).to_be_bytes());
    hash.update(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    for formula in [
        DECOMPOSITION_D_FORMULA_V2,
        DECOMPOSITION_S_FORMULA_V2,
        CANONICAL_VALUE_FORMULA_V2,
        DIGIT_TABLE_FORMULA_V2,
        TOP_BIT_FORMULA_V2,
        LOOKUP_FORMULA_V2,
        LOOKUP_SOUNDNESS_FORMULA_V2,
        CROSS_BASIS_HVZK_FORMULA_V2,
        STATIC_EVIDENCE_FORMULA_V2,
    ] {
        hash.update(&(formula.len() as u16).to_be_bytes());
        hash.update(formula);
    }
    nonzero_digest_v2(hash.finalize())
}
fn exact_accounting_digest_v2() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(RADIX_RANGE_ACCOUNTING_DOMAIN_V2);
    hash.update(&[RADIX_RANGE_VERSION_V2]);
    for value in [
        RADIX_RANGE_WIRE_BYTES_V2 as u64,
        Q_PCS_WIRE_BYTES_V2 as u64,
        RADIX_Q_PCS_COMBINED_WIRE_BYTES_V2 as u64,
        RADIX_Q_PCS_COMBINED_CAP_BYTES_V2 as u64,
        RADIX_Q_PCS_COMBINED_MARGIN_BYTES_V2 as u64,
        RADIX_DIGIT_SLACK_EMISSIONS_V2,
        RADIX_BATCH_INVERSIONS_MAX_V2,
        RADIX_INVERSE_PASS_MULTIPLICATIONS_V2,
        RADIX_FIXED_BASE_SOURCE_RANGE_TERMS_V2,
        RADIX_FIXED_BASE_TERMINAL_TERMS_V2,
        RADIX_SUMCHECK_VISITS_V2,
        RADIX_PACKING_TRANSPOSE_STAGES_V2,
        RADIX_EXTERNAL_IO_BYTES_V2,
        RADIX_CONFIDENTIAL_SCRATCH_BYTES_V2,
        RADIX_SOURCE_PUBLICATION_BYTES_V2,
        Q_PCS_EXTERNAL_PEAK_BYTES_V2,
        RADIX_LOCAL_HEAP_BYTES_V2 as u64,
        RADIX_RETAINED_SOURCE_ROOT_BYTES_V2 as u64,
        RADIX_PHASE_NAMED_HEAP_BYTES_V2 as u64,
        Q_PCS_CONSERVATIVE_HEAP_BYTES_V2 as u64,
        RADIX_Q_PCS_OVERLAP_HEAP_BYTES_V2 as u64,
        RADIX_Q_PCS_HEAP_CEILING_BYTES_V2 as u64,
        RADIX_Q_PCS_HEAP_MARGIN_BYTES_V2 as u64,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&RADIX_LOOKUP_SOUNDNESS_NUMERATOR_V2.to_be_bytes());
    hash.update(&RADIX_LOOKUP_SOUNDNESS_BITS_X100_FLOOR_V2.to_be_bytes());
    hash.update(&CROSS_BASIS_STATISTICAL_HVZK_BITS_V2.to_be_bytes());
    nonzero_digest_v2(hash.finalize())
}
struct RadixRangeManifestCursorV2<'a> {
    encoded: &'a [u8],
    next_ordinal: usize,
    hash: Keccak256,
}
impl<'a> RadixRangeManifestCursorV2<'a> {
    fn begin_v2(encoded: &'a [u8]) -> Self {
        let mut hash = Keccak256::new();
        hash.update(RADIX_RANGE_TRANSCRIPT_DOMAIN_V2);
        hash.update(&[RADIX_RANGE_VERSION_V2]);
        hash.update(&(RADIX_RANGE_TRANSCRIPT_FRAMES_V2.len() as u16).to_be_bytes());
        Self {
            encoded,
            next_ordinal: 0,
            hash,
        }
    }
    fn absorb_expected_v2(&mut self) -> Result<(), ZkAmsMkheErrorV1> {
        let expected = RADIX_RANGE_TRANSCRIPT_FRAMES_V2
            .get(self.next_ordinal)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if self.encoded.len() < 4 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let observed_ordinal = u16::from_be_bytes([self.encoded[0], self.encoded[1]]);
        let length = usize::from(u16::from_be_bytes([self.encoded[2], self.encoded[3]]));
        if observed_ordinal != self.next_ordinal as u16
            || length != expected.len()
            || self.encoded.len() < 4 + length
            || &self.encoded[4..4 + length] != *expected
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        self.hash.update(&self.encoded[..4 + length]);
        self.encoded = &self.encoded[4 + length..];
        self.next_ordinal += 1;
        Ok(())
    }
    fn absorb_until_v2(&mut self, end_exclusive: usize) -> Result<(), ZkAmsMkheErrorV1> {
        while self.next_ordinal < end_exclusive {
            self.absorb_expected_v2()?;
        }
        if self.next_ordinal != end_exclusive {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }
}
struct RadixRangePreLookupManifestV2<'a> {
    cursor: RadixRangeManifestCursorV2<'a>,
}
struct RadixRangeLookupZDerivedManifestV2<'a> {
    cursor: RadixRangeManifestCursorV2<'a>,
}
struct RadixRangeLookupUManifestV2<'a> {
    cursor: RadixRangeManifestCursorV2<'a>,
}
impl<'a> RadixRangePreLookupManifestV2<'a> {
    fn begin_v2(encoded: &'a [u8]) -> Self {
        Self {
            cursor: RadixRangeManifestCursorV2::begin_v2(encoded),
        }
    }
    fn derive_lookup_z_v2(
        mut self,
    ) -> Result<RadixRangeLookupZDerivedManifestV2<'a>, ZkAmsMkheErrorV1> {
        // Frame 19 derives z outside the digit table. No inverse frame is
        // accepted by this pre-z state.
        self.cursor.absorb_until_v2(20)?;
        Ok(RadixRangeLookupZDerivedManifestV2 {
            cursor: self.cursor,
        })
    }
}
impl<'a> RadixRangeLookupZDerivedManifestV2<'a> {
    fn absorb_z_dependent_inverse_planes_v2(
        mut self,
    ) -> Result<RadixRangeLookupUManifestV2<'a>, ZkAmsMkheErrorV1> {
        // Only this post-challenge state accepts the D-inverse and S-inverse
        // planes. Their completion is the U state; there is no third frame.
        self.cursor.absorb_until_v2(22)?;
        Ok(RadixRangeLookupUManifestV2 {
            cursor: self.cursor,
        })
    }
}
impl RadixRangeLookupUManifestV2<'_> {
    fn finish_v2(mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        self.cursor
            .absorb_until_v2(RADIX_RANGE_TRANSCRIPT_FRAMES_V2.len())?;
        if !self.cursor.encoded.is_empty() {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        nonzero_digest_v2(self.cursor.hash.finalize())
    }
}
fn require_exact_transcript_manifest_v2(encoded: &[u8]) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    RadixRangePreLookupManifestV2::begin_v2(encoded)
        .derive_lookup_z_v2()?
        .absorb_z_dependent_inverse_planes_v2()?
        .finish_v2()
}
/// Production cannot assert that the source-algebra witness has been
/// materialized and bound to the exact 43-record source order.
pub(super) enum RadixRangeSourceSealV2 {
    Production {
        source_algebra: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
/// Production cannot open an authenticated, purpose-bound replay view.
pub(super) enum RadixRangeReplaySealV2 {
    Production {
        authenticated_replay: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
/// Production cannot assert the coefficient/packing transpose or equality.
pub(super) enum RadixRangePackingSealV2 {
    Production {
        packing_transpose: Infallible,
        packing_equality: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
/// Production cannot assert the lookup, sumcheck, Hyrax, or statistical-HVZK
/// obligations represented by this static plan.
pub(super) enum RadixRangeZkSealV2 {
    Production {
        radix_lookup: Infallible,
        sumcheck: Infallible,
        hyrax: Infallible,
        statistical_hvzk: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
#[cfg(test)]
static ZEROIZED_TRANSIENT_DROPS_V2: AtomicUsize = AtomicUsize::new(0);
struct RadixRangeTransientV2 {
    bytes: [u8; 32],
}
impl Drop for RadixRangeTransientV2 {
    fn drop(&mut self) {
        self.bytes.fill(0);
        #[cfg(test)]
        if self.bytes.iter().all(|byte| *byte == 0) {
            ZEROIZED_TRANSIENT_DROPS_V2.fetch_add(1, Ordering::SeqCst);
        }
    }
}
struct RadixRangeLiveV2 {
    _source: RadixRangeSourceSealV2,
    _replay: RadixRangeReplaySealV2,
    _packing: RadixRangePackingSealV2,
    _zk: RadixRangeZkSealV2,
    transient: RadixRangeTransientV2,
}
struct RadixRangeIngressV2 {
    live: Option<RadixRangeLiveV2>,
}
struct RadixRangeCheckedV2 {
    live: Option<RadixRangeLiveV2>,
    topology_digest: [u8; 32],
    formula_digest: [u8; 32],
    accounting_digest: [u8; 32],
    transcript_manifest_digest: [u8; 32],
}
struct RadixRangeStaticRecordV2 {
    topology_digest: [u8; 32],
    formula_digest: [u8; 32],
    accounting_digest: [u8; 32],
    transcript_manifest_digest: [u8; 32],
    source_algebra_actually_verified: bool,
    radix_decomposition_actually_verified: bool,
    radix_canonical_range_actually_verified: bool,
    radix_lookup_actually_verified: bool,
    zero_knowledge_actually_verified: bool,
    packing_transpose_actually_verified: bool,
    packing_equality_actually_verified: bool,
    hyrax_actually_verified: bool,
    q_pcs_replay_actually_verified: bool,
    q_pcs_handoff_actually_complete: bool,
    operational_qualification_accepted: bool,
    receipt_accepted: bool,
    rss_qualified: bool,
    proof_minted: bool,
    authority_minted: bool,
    release_complete: bool,
    record_digest: [u8; 32],
}
/// Move-only owner of static planning evidence.  It exposes no decomposition
/// seam and carries no proof or release authority.
pub(super) struct Phase23RadixRangeStaticPrerequisiteV2 {
    live: Option<RadixRangeLiveV2>,
    record: RadixRangeStaticRecordV2,
}
impl RadixRangeIngressV2 {
    fn begin_v2(
        source: RadixRangeSourceSealV2,
        replay: RadixRangeReplaySealV2,
        packing: RadixRangePackingSealV2,
        zk: RadixRangeZkSealV2,
    ) -> Self {
        Self {
            live: Some(RadixRangeLiveV2 {
                _source: source,
                _replay: replay,
                _packing: packing,
                _zk: zk,
                transient: RadixRangeTransientV2 { bytes: [0xa5; 32] },
            }),
        }
    }
    fn check_v2(
        mut self,
        transcript_manifest: &[u8],
    ) -> Result<RadixRangeCheckedV2, ZkAmsMkheErrorV1> {
        // Poison before every validation.  An error or unwind drops all four
        // move-only seals and the zeroizing transient in this stack frame.
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let transcript_manifest_digest = require_exact_transcript_manifest_v2(transcript_manifest)?;
        let topology_digest = exact_topology_digest_v2()?;
        let formula_digest = exact_formula_digest_v2()?;
        let accounting_digest = exact_accounting_digest_v2()?;
        live.transient.bytes = transcript_manifest_digest;
        Ok(RadixRangeCheckedV2 {
            live: Some(live),
            topology_digest,
            formula_digest,
            accounting_digest,
            transcript_manifest_digest,
        })
    }
    #[cfg(test)]
    fn force_unwind_after_take_v2(mut self) -> ! {
        let _live = self.live.take().expect("test ingress must be live");
        panic!("test unwind after poison")
    }
}
impl RadixRangeCheckedV2 {
    fn freeze_v2(mut self) -> Result<Phase23RadixRangeStaticPrerequisiteV2, ZkAmsMkheErrorV1> {
        let live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        for digest in [
            self.topology_digest,
            self.formula_digest,
            self.accounting_digest,
            self.transcript_manifest_digest,
        ] {
            nonzero_digest_v2(digest)?;
        }
        let mut record = RadixRangeStaticRecordV2 {
            topology_digest: self.topology_digest,
            formula_digest: self.formula_digest,
            accounting_digest: self.accounting_digest,
            transcript_manifest_digest: self.transcript_manifest_digest,
            source_algebra_actually_verified: SOURCE_ALGEBRA_ACTUALLY_VERIFIED_V2,
            radix_decomposition_actually_verified: RADIX_DECOMPOSITION_ACTUALLY_VERIFIED_V2,
            radix_canonical_range_actually_verified: RADIX_CANONICAL_RANGE_ACTUALLY_VERIFIED_V2,
            radix_lookup_actually_verified: RADIX_LOOKUP_ACTUALLY_VERIFIED_V2,
            zero_knowledge_actually_verified: ZERO_KNOWLEDGE_ACTUALLY_VERIFIED_V2,
            packing_transpose_actually_verified: PACKING_TRANSPOSE_ACTUALLY_VERIFIED_V2,
            packing_equality_actually_verified: PACKING_EQUALITY_ACTUALLY_VERIFIED_V2,
            hyrax_actually_verified: HYRAX_ACTUALLY_VERIFIED_V2,
            q_pcs_replay_actually_verified: Q_PCS_REPLAY_ACTUALLY_VERIFIED_V2,
            q_pcs_handoff_actually_complete: Q_PCS_HANDOFF_ACTUALLY_COMPLETE_V2,
            operational_qualification_accepted: OPERATIONAL_QUALIFICATION_ACCEPTED_V2,
            receipt_accepted: RECEIPT_ACCEPTED_V2,
            rss_qualified: RSS_QUALIFIED_V2,
            proof_minted: PROOF_MINTED_V2,
            authority_minted: AUTHORITY_MINTED_V2,
            release_complete: RELEASE_COMPLETE_V2,
            record_digest: [0; 32],
        };
        record.record_digest = static_record_digest_v2(&record)?;
        validate_static_record_v2(&record)?;
        Ok(Phase23RadixRangeStaticPrerequisiteV2 {
            live: Some(live),
            record,
        })
    }
}
fn static_record_digest_v2(
    record: &RadixRangeStaticRecordV2,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(RADIX_RANGE_PREREQUISITE_DOMAIN_V2);
    hash.update(&[RADIX_RANGE_VERSION_V2]);
    hash.update(&record.topology_digest);
    hash.update(&record.formula_digest);
    hash.update(&record.accounting_digest);
    hash.update(&record.transcript_manifest_digest);
    hash.update(&[
        record.source_algebra_actually_verified as u8,
        record.radix_decomposition_actually_verified as u8,
        record.radix_canonical_range_actually_verified as u8,
        record.radix_lookup_actually_verified as u8,
        record.zero_knowledge_actually_verified as u8,
        record.packing_transpose_actually_verified as u8,
        record.packing_equality_actually_verified as u8,
        record.hyrax_actually_verified as u8,
        record.q_pcs_replay_actually_verified as u8,
        record.q_pcs_handoff_actually_complete as u8,
        record.operational_qualification_accepted as u8,
        record.receipt_accepted as u8,
        record.rss_qualified as u8,
        record.proof_minted as u8,
        record.authority_minted as u8,
        record.release_complete as u8,
    ]);
    nonzero_digest_v2(hash.finalize())
}
fn validate_static_record_v2(record: &RadixRangeStaticRecordV2) -> Result<(), ZkAmsMkheErrorV1> {
    if [
        record.topology_digest,
        record.formula_digest,
        record.accounting_digest,
        record.transcript_manifest_digest,
        record.record_digest,
    ]
    .contains(&[0; 32])
        || record.source_algebra_actually_verified
        || record.radix_decomposition_actually_verified
        || record.radix_canonical_range_actually_verified
        || record.radix_lookup_actually_verified
        || record.zero_knowledge_actually_verified
        || record.packing_transpose_actually_verified
        || record.packing_equality_actually_verified
        || record.hyrax_actually_verified
        || record.q_pcs_replay_actually_verified
        || record.q_pcs_handoff_actually_complete
        || record.operational_qualification_accepted
        || record.receipt_accepted
        || record.rss_qualified
        || record.proof_minted
        || record.authority_minted
        || record.release_complete
        || record.record_digest != static_record_digest_v2(record)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}
/// Consumes four impossible production seals into static evidence only.
pub(super) fn consume_phase23_radix_range_static_prerequisite_v2(
    source: RadixRangeSourceSealV2,
    replay: RadixRangeReplaySealV2,
    packing: RadixRangePackingSealV2,
    zk: RadixRangeZkSealV2,
    transcript_manifest: &[u8],
) -> Result<Phase23RadixRangeStaticPrerequisiteV2, ZkAmsMkheErrorV1> {
    RadixRangeIngressV2::begin_v2(source, replay, packing, zk)
        .check_v2(transcript_manifest)?
        .freeze_v2()
}
#[cfg(test)]
#[path = "incremental_source_phase23_radix_range_v2_tests.rs"]
mod tests;
