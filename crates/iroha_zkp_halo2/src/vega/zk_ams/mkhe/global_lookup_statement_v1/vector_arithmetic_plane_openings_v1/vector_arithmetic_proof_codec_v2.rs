//! Inert exact codec contract for vector-arithmetic statements 3, 5, and 8.
//!
//! This freezes only an aggregate manifest and borrowed syntax parser. It adds
//! no circuit, transcript, proof execution, or acceptance authority; the
//! backend prerequisite and production seal remain uninhabited.

#![allow(dead_code, reason = "production acceptance remains uninhabited")]

use core::convert::Infallible;

use crate::vega::{
    VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
    bulletproof_t256::ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1, sponge::Keccak256,
};

use super::{
    challenge_manifest_digest_v1, global_lookup_topology_digest_v1, plane_mapping_digest_v1,
};

const VECTOR_ARITHMETIC_CODEC_VERSION_V2: u8 = 1;
const VECTOR_ARITHMETIC_PROOF_COUNT_V2: usize = 3;
const VECTOR_ARITHMETIC_ENVELOPE_MAGIC_V2: [u8; 4] = *b"ZGVA";
const VECTOR_ARITHMETIC_ENVELOPE_FLAGS_V2: u8 = 0;
const VECTOR_ARITHMETIC_ENVELOPE_BYTES_V2: usize = 50;
const POINT_BYTES_V2: usize = 33;
const SCALAR_BYTES_V2: usize = 32;
const COORDINATE_COUNT_V2: u32 = 16_384;
const GENERALIZED_BP_FIXED_POINTS_V2: u32 = 7;
const GENERALIZED_BP_SCALARS_V2: u32 = 5;
const CURRENT_T256_BP_MAX_GATES_V2: u32 = 65_536;

const KNOWN_BASE_WIRE_BYTES_V2: u32 = 32_844_686;
const VECTOR_ARITHMETIC_SECTION_WIRE_BYTES_V2: u32 = 642_315;
const CONDITIONAL_TOTAL_WIRE_BYTES_V2: u32 = 33_487_001;
const RELEASE_WIRE_CAP_BYTES_V2: u32 = 33_554_432;
const CONDITIONAL_WIRE_MARGIN_BYTES_V2: u32 = 67_431;

const VECTOR_ARITHMETIC_MANIFEST_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.vector-arithmetic-proof.manifest\0";
const EQUATION_LANGUAGE_V2: &[u8] = b"for-v=0..16383;q3[v]=sum_g=0..343(kappa^g*(bD_g[v]*(bD_g[v]-1)+delta*bS_g[v]*(bS_g[v]-1)+delta^2*bD_g[v]*bS_g[v]));q5[v]=sum_g=0..343(kappa^g*(sum_h=0..17(delta^h*beta_g,h[v]*(beta_g,h[v]-1))+delta^18*(m_g[v]-bD_g[v]*beta_g,16[v])+delta^19*(beta_g,17[v]-beta_g,16[v]+m_g[v])));q8[v]=sum_u=0..1031(kappa^u*((x_u[v]+n_u[v])*n_u[v]))";
const DENSE_MAPPING_ORDER_LANGUAGE_V2: &[u8] = b"logical-plane-to-commitment-session-physical-role:bD[0..343]->[12040..12383];bS[344..687]->[12384..12727];beta[688..6879]->[18576..24767];m[6880..7223]->[24768..25111];x[7224..8255]->[25112..26143];n[8256..9287]->[26144..27175];q3,q5,q8[9288..9290]->[71106..71108];9291-logical-planes;no-inverse-roles";
const COMMITMENT_GATE_ORDER_LANGUAGE_V2: &[u8] = b"proof-order=S3,S5,S8;commitment-order=S3=(bD[0..343],bS[0..343],q3),S5=(bD[0..343],beta[group-major][0..17],m[0..343],q5),S8=(x[0..1031],n[0..1031],q8);gate-order=coordinate-v-major;S3-then-group-major-then-(bD-boolean,bS-boolean,bD-times-bS);S5-then-group-major-then-(beta-boolean-h=0..17,bD-times-beta16);S8-then-unit-major-(x-plus-n)-times-n;constraint-order=two-input-links-per-multiplication-in-gate-order-then-one-output-aggregate-per-coordinate;constraint-count=2*actual-gates+16384;padded-gates-have-no-logical-plane-or-extra-aggregate";
const ENVELOPE_LANGUAGE_V2: &[u8] = b"one-aggregate-purpose-manifest-shared-by-all-three-envelopes;ZGVA||codec-version:u8=1||flags:u8=0||statement:u8||logP:u8||C:u16be||actual-gates:u32be||core-len:u32be||aggregate-purpose-manifest:[u8;32];exactly-50-bytes;wire=50-byte-envelope||raw-generalized-Bulletproof-core;core-order=(2*C+7)-nonidentity-points,3-canonical-scalars,2*logP-nonidentity-IPA-points,2-canonical-final-scalars;exact-consumption";
const BACKEND_ACCOUNTING_LANGUAGE_V2: &[u8] = b"current-T256-generalized-Bulletproof-basis-cap=65536-gates;required-padded-gates=(33554432,134217728,33554432);current-eager-backend-cannot-instantiate-any-shape;raising-only-the-cap-is-insufficient;requires-streaming-sparse-tensor-aware-backend-or-new-product-argument;chunking-at-65536-requires-2150-cores-and-at-least-3252950B>709746B-pre-envelope-room;raw-cores=(47515,456319,138331);three-50B-envelopes;known-base=32844686;section=642315;conditional-total=33487001;cap=33554432;margin=67431;about-226.42-bit-simple-union-estimate-is-not-an-accepted-soundness-theorem;all-readiness-gates-zero";

const PINNED_TOPOLOGY_DIGEST_V2: [u8; 32] = [
    0x3a, 0xf9, 0xa6, 0xad, 0x67, 0x38, 0x3c, 0x32, 0xb0, 0x6b, 0xb5, 0xd9, 0x5a, 0x05, 0x86, 0x3b,
    0x8c, 0xb0, 0xb3, 0x33, 0x86, 0x60, 0x17, 0x7b, 0xc2, 0xa9, 0x2e, 0x1b, 0xbf, 0x40, 0xb4, 0xab,
];
const PINNED_CHALLENGE_MANIFEST_DIGEST_V2: [u8; 32] = [
    0xe3, 0x73, 0x09, 0x11, 0x78, 0x5c, 0xb1, 0xe2, 0x33, 0x32, 0xee, 0x9a, 0x13, 0x61, 0x81, 0x0c,
    0x43, 0x5f, 0x76, 0xb9, 0x3b, 0xec, 0xd5, 0x4e, 0x3b, 0x0d, 0x18, 0x96, 0x44, 0xb3, 0x2d, 0x99,
];
const PINNED_T256_BP_BASIS_DIGEST_V2: [u8; 32] = [
    0xbf, 0x81, 0xc8, 0x30, 0x91, 0xa4, 0x26, 0xbb, 0xcb, 0x2f, 0x75, 0x18, 0xad, 0x37, 0x16, 0x39,
    0x18, 0x10, 0xe5, 0x0b, 0x84, 0x8b, 0x38, 0xd0, 0xc2, 0xb3, 0xcd, 0x96, 0xaf, 0xf9, 0xa3, 0xf8,
];
const PINNED_PLANE_MAPPING_DIGEST_V2: [u8; 32] = [
    0x68, 0x9d, 0xc6, 0xe0, 0x84, 0x12, 0x87, 0xf2, 0xab, 0x74, 0xc8, 0x13, 0x66, 0xff, 0x64, 0xcd,
    0x5a, 0x49, 0x0f, 0xaf, 0xfe, 0x0b, 0x1d, 0x35, 0xbb, 0x69, 0x09, 0x0d, 0xbb, 0xe7, 0x64, 0xd9,
];

const VECTOR_ARITHMETIC_PROFILE_ACCEPTED_V2: bool = false;
const VECTOR_ARITHMETIC_PLANE_OPENINGS_MATERIALIZED_V2: bool = false;
const VECTOR_ARITHMETIC_STREAMING_BACKEND_READY_V2: bool = false;
const VECTOR_ARITHMETIC_PROVER_WIRED_V2: bool = false;
const VECTOR_ARITHMETIC_VERIFIER_WIRED_V2: bool = false;
const VECTOR_ARITHMETIC_PROOF_VERIFIED_V2: bool = false;
const VECTOR_ARITHMETIC_SOUNDNESS_ACCEPTED_V2: bool = false;
const VECTOR_ARITHMETIC_ZERO_KNOWLEDGE_ACCEPTED_V2: bool = false;
const VECTOR_ARITHMETIC_RECEIPT_ACCEPTED_V2: bool = false;
const VECTOR_ARITHMETIC_AUTHORITY_MINTED_V2: bool = false;
const VECTOR_ARITHMETIC_RSS_QUALIFIED_V2: bool = false;
const VECTOR_ARITHMETIC_RELEASE_READY_V2: bool = false;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VectorArithmeticCodecErrorV2 {
    Shape,
    Order,
    Context,
    WireEncoding,
    PointEncoding,
    ScalarEncoding,
    Resource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct VectorArithmeticProofShapeV2 {
    ordinal: u8,
    statement: u8,
    log_padded_gates: u8,
    commitment_count: u16,
    actual_gates: u32,
    padded_gates: u32,
    padding_gates: u32,
    constraint_count: u32,
    core_bytes: u32,
    wire_bytes: u32,
}

const PROOF_SHAPES_V2: [VectorArithmeticProofShapeV2; VECTOR_ARITHMETIC_PROOF_COUNT_V2] = [
    VectorArithmeticProofShapeV2 {
        ordinal: 0,
        statement: 3,
        log_padded_gates: 25,
        commitment_count: 689,
        actual_gates: 16_908_288,
        padded_gates: 33_554_432,
        padding_gates: 16_646_144,
        constraint_count: 33_832_960,
        core_bytes: 47_515,
        wire_bytes: 47_565,
    },
    VectorArithmeticProofShapeV2 {
        ordinal: 1,
        statement: 5,
        log_padded_gates: 27,
        commitment_count: 6_881,
        actual_gates: 107_085_824,
        padded_gates: 134_217_728,
        padding_gates: 27_131_904,
        constraint_count: 214_188_032,
        core_bytes: 456_319,
        wire_bytes: 456_369,
    },
    VectorArithmeticProofShapeV2 {
        ordinal: 2,
        statement: 8,
        log_padded_gates: 25,
        commitment_count: 2_065,
        actual_gates: 16_908_288,
        padded_gates: 33_554_432,
        padding_gates: 16_646_144,
        constraint_count: 33_832_960,
        core_bytes: 138_331,
        wire_bytes: 138_381,
    },
];

const fn readiness_gates_v2() -> [u8; 12] {
    [
        VECTOR_ARITHMETIC_PROFILE_ACCEPTED_V2 as u8,
        VECTOR_ARITHMETIC_PLANE_OPENINGS_MATERIALIZED_V2 as u8,
        VECTOR_ARITHMETIC_STREAMING_BACKEND_READY_V2 as u8,
        VECTOR_ARITHMETIC_PROVER_WIRED_V2 as u8,
        VECTOR_ARITHMETIC_VERIFIER_WIRED_V2 as u8,
        VECTOR_ARITHMETIC_PROOF_VERIFIED_V2 as u8,
        VECTOR_ARITHMETIC_SOUNDNESS_ACCEPTED_V2 as u8,
        VECTOR_ARITHMETIC_ZERO_KNOWLEDGE_ACCEPTED_V2 as u8,
        VECTOR_ARITHMETIC_RECEIPT_ACCEPTED_V2 as u8,
        VECTOR_ARITHMETIC_AUTHORITY_MINTED_V2 as u8,
        VECTOR_ARITHMETIC_RSS_QUALIFIED_V2 as u8,
        VECTOR_ARITHMETIC_RELEASE_READY_V2 as u8,
    ]
}

const _: () = {
    assert!(CURRENT_T256_BP_MAX_GATES_V2 == 65_536);
    assert!(VECTOR_ARITHMETIC_ENVELOPE_BYTES_V2 == 50);
    assert!(VECTOR_ARITHMETIC_SECTION_WIRE_BYTES_V2 == 642_315);
    assert!(
        KNOWN_BASE_WIRE_BYTES_V2 + VECTOR_ARITHMETIC_SECTION_WIRE_BYTES_V2
            == CONDITIONAL_TOTAL_WIRE_BYTES_V2
    );
    assert!(
        RELEASE_WIRE_CAP_BYTES_V2 - CONDITIONAL_TOTAL_WIRE_BYTES_V2
            == CONDITIONAL_WIRE_MARGIN_BYTES_V2
    );
};

fn proof_shape_v2(
    ordinal: usize,
) -> Result<VectorArithmeticProofShapeV2, VectorArithmeticCodecErrorV2> {
    PROOF_SHAPES_V2
        .get(ordinal)
        .copied()
        .ok_or(VectorArithmeticCodecErrorV2::Order)
}

fn generalized_bp_core_bytes_v2(
    shape: VectorArithmeticProofShapeV2,
) -> Result<u32, VectorArithmeticCodecErrorV2> {
    let points = u32::from(shape.commitment_count)
        .checked_mul(2)
        .and_then(|value| value.checked_add(GENERALIZED_BP_FIXED_POINTS_V2))
        .and_then(|value| value.checked_add(2 * u32::from(shape.log_padded_gates)))
        .ok_or(VectorArithmeticCodecErrorV2::Resource)?;
    points
        .checked_mul(POINT_BYTES_V2 as u32)
        .and_then(|value| value.checked_add(GENERALIZED_BP_SCALARS_V2 * SCALAR_BYTES_V2 as u32))
        .ok_or(VectorArithmeticCodecErrorV2::Resource)
}

fn validate_static_profile_v2() -> Result<(), VectorArithmeticCodecErrorV2> {
    if global_lookup_topology_digest_v1() != PINNED_TOPOLOGY_DIGEST_V2
        || challenge_manifest_digest_v1() != PINNED_CHALLENGE_MANIFEST_DIGEST_V2
        || ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1 != PINNED_T256_BP_BASIS_DIGEST_V2
        || plane_mapping_digest_v1().map_err(|_| VectorArithmeticCodecErrorV2::Context)?
            != PINNED_PLANE_MAPPING_DIGEST_V2
    {
        return Err(VectorArithmeticCodecErrorV2::Context);
    }
    let mut section = 0_u32;
    for (ordinal, shape) in PROOF_SHAPES_V2.iter().copied().enumerate() {
        if usize::from(shape.ordinal) != ordinal
            || shape.padded_gates != 1_u32 << shape.log_padded_gates
            || shape.padding_gates != shape.padded_gates - shape.actual_gates
            || shape.constraint_count != 2 * shape.actual_gates + COORDINATE_COUNT_V2
            || shape.core_bytes != generalized_bp_core_bytes_v2(shape)?
            || shape.wire_bytes != shape.core_bytes + VECTOR_ARITHMETIC_ENVELOPE_BYTES_V2 as u32
            || shape.padded_gates <= CURRENT_T256_BP_MAX_GATES_V2
        {
            return Err(VectorArithmeticCodecErrorV2::Shape);
        }
        section = section
            .checked_add(shape.wire_bytes)
            .ok_or(VectorArithmeticCodecErrorV2::Resource)?;
    }
    if section != VECTOR_ARITHMETIC_SECTION_WIRE_BYTES_V2 {
        return Err(VectorArithmeticCodecErrorV2::Shape);
    }
    Ok(())
}

fn absorb_len_prefixed_v2(
    hash: &mut Keccak256,
    bytes: &[u8],
) -> Result<(), VectorArithmeticCodecErrorV2> {
    let len = u16::try_from(bytes.len()).map_err(|_| VectorArithmeticCodecErrorV2::Resource)?;
    hash.update(&len.to_be_bytes());
    hash.update(bytes);
    Ok(())
}

fn vector_arithmetic_manifest_digest_v2() -> Result<[u8; 32], VectorArithmeticCodecErrorV2> {
    validate_static_profile_v2()?;
    let mut hash = Keccak256::new();
    hash.update(VECTOR_ARITHMETIC_MANIFEST_DOMAIN_V2);
    hash.update(&[VECTOR_ARITHMETIC_CODEC_VERSION_V2]);
    hash.update(&PINNED_TOPOLOGY_DIGEST_V2);
    hash.update(&PINNED_CHALLENGE_MANIFEST_DIGEST_V2);
    hash.update(&PINNED_T256_BP_BASIS_DIGEST_V2);
    hash.update(&PINNED_PLANE_MAPPING_DIGEST_V2);
    for language in [
        EQUATION_LANGUAGE_V2,
        DENSE_MAPPING_ORDER_LANGUAGE_V2,
        COMMITMENT_GATE_ORDER_LANGUAGE_V2,
        ENVELOPE_LANGUAGE_V2,
        BACKEND_ACCOUNTING_LANGUAGE_V2,
    ] {
        absorb_len_prefixed_v2(&mut hash, language)?;
    }
    hash.update(&[VECTOR_ARITHMETIC_PROOF_COUNT_V2 as u8]);
    for shape in PROOF_SHAPES_V2 {
        hash.update(&[shape.ordinal, shape.statement, shape.log_padded_gates]);
        hash.update(&shape.commitment_count.to_be_bytes());
        hash.update(&shape.actual_gates.to_be_bytes());
        hash.update(&shape.padding_gates.to_be_bytes());
        hash.update(&shape.constraint_count.to_be_bytes());
        hash.update(&shape.core_bytes.to_be_bytes());
        hash.update(&(VECTOR_ARITHMETIC_ENVELOPE_BYTES_V2 as u16).to_be_bytes());
        hash.update(&shape.wire_bytes.to_be_bytes());
    }
    for value in [
        KNOWN_BASE_WIRE_BYTES_V2,
        VECTOR_ARITHMETIC_SECTION_WIRE_BYTES_V2,
        CONDITIONAL_TOTAL_WIRE_BYTES_V2,
        RELEASE_WIRE_CAP_BYTES_V2,
        CONDITIONAL_WIRE_MARGIN_BYTES_V2,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&readiness_gates_v2());
    let digest = hash.finalize();
    (digest != [0; 32])
        .then_some(digest)
        .ok_or(VectorArithmeticCodecErrorV2::Context)
}

fn write_vector_arithmetic_envelope_v2(
    output: &mut [u8; VECTOR_ARITHMETIC_ENVELOPE_BYTES_V2],
    ordinal: usize,
) -> Result<(), VectorArithmeticCodecErrorV2> {
    let shape = proof_shape_v2(ordinal)?;
    output.fill(0);
    output[..4].copy_from_slice(&VECTOR_ARITHMETIC_ENVELOPE_MAGIC_V2);
    output[4] = VECTOR_ARITHMETIC_CODEC_VERSION_V2;
    output[5] = VECTOR_ARITHMETIC_ENVELOPE_FLAGS_V2;
    output[6] = shape.statement;
    output[7] = shape.log_padded_gates;
    output[8..10].copy_from_slice(&shape.commitment_count.to_be_bytes());
    output[10..14].copy_from_slice(&shape.actual_gates.to_be_bytes());
    output[14..18].copy_from_slice(&shape.core_bytes.to_be_bytes());
    output[18..].copy_from_slice(&vector_arithmetic_manifest_digest_v2()?);
    Ok(())
}

struct CanonicalCoreCursorV2<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl CanonicalCoreCursorV2<'_> {
    fn read_point_v2(&mut self) -> Result<(), VectorArithmeticCodecErrorV2> {
        let end = self
            .cursor
            .checked_add(POINT_BYTES_V2)
            .ok_or(VectorArithmeticCodecErrorV2::Resource)?;
        Point::from_non_identity_wire_bytes_exact(
            self.bytes
                .get(self.cursor..end)
                .ok_or(VectorArithmeticCodecErrorV2::WireEncoding)?,
        )
        .map_err(|_| VectorArithmeticCodecErrorV2::PointEncoding)?;
        self.cursor = end;
        Ok(())
    }

    fn read_scalar_v2(&mut self) -> Result<(), VectorArithmeticCodecErrorV2> {
        let end = self
            .cursor
            .checked_add(SCALAR_BYTES_V2)
            .ok_or(VectorArithmeticCodecErrorV2::Resource)?;
        let encoded = self
            .bytes
            .get(self.cursor..end)
            .ok_or(VectorArithmeticCodecErrorV2::WireEncoding)?
            .try_into()
            .map_err(|_| VectorArithmeticCodecErrorV2::ScalarEncoding)?;
        Scalar::from_le_bytes_exact(encoded)
            .map_err(|_| VectorArithmeticCodecErrorV2::ScalarEncoding)?;
        self.cursor = end;
        Ok(())
    }

    fn finish_v2(self) -> Result<(), VectorArithmeticCodecErrorV2> {
        (self.cursor == self.bytes.len())
            .then_some(())
            .ok_or(VectorArithmeticCodecErrorV2::WireEncoding)
    }
}

struct BorrowedVectorArithmeticProofV2<'a> {
    shape: VectorArithmeticProofShapeV2,
    envelope: &'a [u8; VECTOR_ARITHMETIC_ENVELOPE_BYTES_V2],
    core: &'a [u8],
}

fn borrow_vector_arithmetic_proof_exact_v2<'a>(
    wire: &'a [u8],
    ordinal: usize,
) -> Result<BorrowedVectorArithmeticProofV2<'a>, VectorArithmeticCodecErrorV2> {
    let shape = proof_shape_v2(ordinal)?;
    if wire.len() != shape.wire_bytes as usize {
        return Err(VectorArithmeticCodecErrorV2::WireEncoding);
    }
    let (envelope, core) = wire.split_at(VECTOR_ARITHMETIC_ENVELOPE_BYTES_V2);
    let envelope: &[u8; VECTOR_ARITHMETIC_ENVELOPE_BYTES_V2] = envelope
        .try_into()
        .map_err(|_| VectorArithmeticCodecErrorV2::WireEncoding)?;
    let mut expected = [0_u8; VECTOR_ARITHMETIC_ENVELOPE_BYTES_V2];
    write_vector_arithmetic_envelope_v2(&mut expected, ordinal)?;
    if *envelope != expected || core.len() != shape.core_bytes as usize {
        return Err(VectorArithmeticCodecErrorV2::WireEncoding);
    }
    let mut cursor = CanonicalCoreCursorV2 {
        bytes: core,
        cursor: 0,
    };
    for _ in 0..(2 * u32::from(shape.commitment_count) + GENERALIZED_BP_FIXED_POINTS_V2) {
        cursor.read_point_v2()?;
    }
    for _ in 0..3 {
        cursor.read_scalar_v2()?;
    }
    for _ in 0..2 * u32::from(shape.log_padded_gates) {
        cursor.read_point_v2()?;
    }
    for _ in 0..2 {
        cursor.read_scalar_v2()?;
    }
    cursor.finish_v2()?;
    Ok(BorrowedVectorArithmeticProofV2 {
        shape,
        envelope,
        core,
    })
}

struct BorrowedVectorArithmeticProofSetV2<'a> {
    proofs: [BorrowedVectorArithmeticProofV2<'a>; VECTOR_ARITHMETIC_PROOF_COUNT_V2],
}

fn borrow_vector_arithmetic_proof_set_exact_v2<'a>(
    wires: [&'a [u8]; VECTOR_ARITHMETIC_PROOF_COUNT_V2],
) -> Result<BorrowedVectorArithmeticProofSetV2<'a>, VectorArithmeticCodecErrorV2> {
    let [s3, s5, s8] = wires;
    Ok(BorrowedVectorArithmeticProofSetV2 {
        proofs: [
            borrow_vector_arithmetic_proof_exact_v2(s3, 0)?,
            borrow_vector_arithmetic_proof_exact_v2(s5, 1)?,
            borrow_vector_arithmetic_proof_exact_v2(s8, 2)?,
        ],
    })
}

enum VectorArithmeticProductionAcceptanceSealV2 {
    Production {
        instantiated_streaming_backend: Infallible,
        verified_equations: Infallible,
        accepted_soundness_theorem: Infallible,
    },
}

fn accept_vector_arithmetic_proof_set_v2(
    _proofs: BorrowedVectorArithmeticProofSetV2<'_>,
    seal: VectorArithmeticProductionAcceptanceSealV2,
) -> Result<[u8; 32], VectorArithmeticCodecErrorV2> {
    let VectorArithmeticProductionAcceptanceSealV2::Production {
        instantiated_streaming_backend,
        ..
    } = seal;
    match instantiated_streaming_backend {}
}

#[cfg(test)]
#[path = "vector_arithmetic_proof_codec_v2_tests.rs"]
mod tests;
