//! Non-authorizing committed-MLE profile and borrowed proof codecs.
//!
//! This child fixes the basis, layout, statement order, transcript purpose,
//! and byte accounting needed by a later global-lookup opening implementation.
//! It owns no polynomial, opening, randomness, prover, verifier, or release
//! authority. Production data-plane and proof-session seals remain uninhabited.
use super::{
    COEFFICIENT_IPAS_V1, ENDPOINT_GATES_V1, ENDPOINT_STATEMENTS_V1, HIDDEN_ENDPOINTS_V1,
    IpaStatementRoleV1, challenge_v1::challenge_manifest_digest_v1,
    global_lookup_topology_digest_v1, ipa_statement_role_v1, ipa_tag_v1,
};
use crate::vega::{
    VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
    bulletproof_t256::ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1, sponge::Keccak256,
};
use core::convert::Infallible;
const COMMITTED_MLE_VERSION_V1: u8 = 1;
const COMMITTED_MLE_MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.committed-mle.manifest\0";
const COMMITTED_MLE_LAYOUT_LANGUAGE_V1: &[u8] =
    b"MLE-index=x=(c_0..c_13,y_0..y_14);bits-little-endian;coordinate-axis-first-14;plane-axis-second-15;plane-vector-basis=G[0..16384);table-vector-basis=G[0..32768);mask-vector-basis=G[0..1024);scalar-endpoints=Pedersen(g,h)";
const COMMITTED_MLE_TRANSCRIPT_LANGUAGE_V1: &[u8] =
    b"outer-opening-state32=verifier-derived-exact-global-lookup-OpeningStage-after-52-ordered-endpoint-commitments-and-nu[0..15],xi;proof-order=coefficient[0..15],table-M,mask,endpoint-gates;inner-seed32=Keccak('iroha.zk-ams.v1.phase23.global-lookup.committed-mle.inner-seed\0'||frames);frame=u16be(label.len)||label||u64be(payload.len)||payload;frame-label-order=(purpose-manifest,outer-opening-state,topology,challenge-manifest,basis,role,ordinal,n,c-vec,c-scalar,vector-commitments,scalar-commitments,evaluation-spec);payloads=(digest32,digest32,digest32,digest32,digest32,u8,u16be,u32be,u16be,u16be,u16be||ordered-nonidentity-point33*,u16be||ordered-nonidentity-point33*,verifier-derived-role-specific-digest32);no-caller-selected-digest";
const COMMITTED_MLE_BP_PRIMITIVE_LANGUAGE_V1: &[u8] =
    b"state='iroha.generalized-bulletproof.t256.transcript.v1'||inner-seed32;push-scalar=0x00||canonical-scalar-le32;push-point=0x01||canonical-nonidentity-point33;challenge(k,a):wide64=Keccak('iroha.generalized-bulletproof.t256.challenge.v1'||state||k:u32be||a:u8||0x00)||Keccak('iroha.generalized-bulletproof.t256.challenge.v1'||state||k:u32be||a:u8||0x01);challenge=reduce-wide-le(wide64);choose-first-nonzero-for-a=0..127;state||=0x02||k:u32be||a:u8||challenge-le32;k+=1;proof-encoding=9-fixed-points||3-scalars||2log2(n)-IPA-points||2-scalars";
const COMMITTED_MLE_OPENING_DIGEST_LANGUAGE_V1: &[u8] =
    b"opening-digest=Keccak('iroha.zk-ams.v1.phase23.global-lookup.committed-mle.openings\0'||purpose-manifest32||outer-opening-state32||proof-count:u16be||entries);entry-order=coefficient[0..15],table-M,mask,endpoint-gates;entry=ordinal:u16be||role:u8||wire-len:u32be||wire;coefficient/table/mask-wire=raw-core;endpoint-gates-wire=47-byte-ZGEP-envelope||787-byte-raw-core;commitments-are-bound-in-outer-state-and-each-inner-seed-and-are-not-re-serialized;opening-digest-is-internally-derived-only";
const COMMITTED_MLE_ACCOUNTING_SCOPE_LANGUAGE_V1: &[u8] =
    b"accounted-wire-bytes=exactly-19-committed-MLE-opening-proofs;coefficient-q_s-residual-commitments=3-outer-transcript-points-already-parent-accounted-as-99-wire-bytes-and-not-re-serialized-here;coefficient-vector-arithmetic-proofs=3-separate-uninstantiated-proofs-with-unknown-wire-bytes-and-excluded-from-27276-byte-opening-subtotal";
const ENDPOINT_ENVELOPE_LANGUAGE_V1: &[u8] =
    b"ZGEP||version:u8||flags:u8||c_vec:u8||log_n:u8||statements:u8||gates:u16be||scalar-endpoints:u16be||core-bytes:u16be||purpose-manifest:[u8;32]";
const PINNED_T256_BP_BASIS_DIGEST_V1: [u8; 32] = [
    0xbf, 0x81, 0xc8, 0x30, 0x91, 0xa4, 0x26, 0xbb, 0xcb, 0x2f, 0x75, 0x18, 0xad, 0x37, 0x16, 0x39,
    0x18, 0x10, 0xe5, 0x0b, 0x84, 0x8b, 0x38, 0xd0, 0xc2, 0xb3, 0xcd, 0x96, 0xaf, 0xf9, 0xa3, 0xf8,
];
const PINNED_TOPOLOGY_DIGEST_V1: [u8; 32] = [
    0x3a, 0xf9, 0xa6, 0xad, 0x67, 0x38, 0x3c, 0x32, 0xb0, 0x6b, 0xb5, 0xd9, 0x5a, 0x05, 0x86, 0x3b,
    0x8c, 0xb0, 0xb3, 0x33, 0x86, 0x60, 0x17, 0x7b, 0xc2, 0xa9, 0x2e, 0x1b, 0xbf, 0x40, 0xb4, 0xab,
];
const PINNED_CHALLENGE_MANIFEST_DIGEST_V1: [u8; 32] = [
    0xe3, 0x73, 0x09, 0x11, 0x78, 0x5c, 0xb1, 0xe2, 0x33, 0x32, 0xee, 0x9a, 0x13, 0x61, 0x81, 0x0c,
    0x43, 0x5f, 0x76, 0xb9, 0x3b, 0xec, 0xd5, 0x4e, 0x3b, 0x0d, 0x18, 0x96, 0x44, 0xb3, 0x2d, 0x99,
];
const COORDINATE_BITS_V1: u8 = 14;
const PLANE_BITS_V1: u8 = 15;
const LOOKUP_BITS_V1: u8 = COORDINATE_BITS_V1 + PLANE_BITS_V1;
const COEFFICIENT_VECTOR_WIDTH_V1: usize = 1 << COORDINATE_BITS_V1;
const TABLE_M_VECTOR_WIDTH_V1: usize = 1 << PLANE_BITS_V1;
const MASK_VECTOR_WIDTH_V1: usize = 1 << 10;
const ENDPOINT_GATE_VECTOR_WIDTH_V1: usize = 1 << 5;
const POINT_BYTES_V1: usize = 33;
const SCALAR_BYTES_V1: usize = 32;
const BP_FIXED_POINTS_C_LE_ONE_V1: usize = 9;
const BP_FIXED_SCALARS_V1: usize = 5;
const COEFFICIENT_CORE_BYTES_V1: usize = 1_381;
const TABLE_M_CORE_BYTES_V1: usize = 1_447;
const MASK_CORE_BYTES_V1: usize = 1_117;
const ENDPOINT_GATE_CORE_BYTES_V1: usize = 787;
const COMMITTED_MLE_PROOFS_V1: usize = COEFFICIENT_IPAS_V1 + 3;
const TABLE_M_COMMITMENT_BYTES_V1: usize = POINT_BYTES_V1;
const MASK_COMMITMENT_BYTES_V1: usize = POINT_BYTES_V1;
const ENDPOINT_COMMITMENTS_BYTES_V1: usize = HIDDEN_ENDPOINTS_V1 * POINT_BYTES_V1;
const ENDPOINT_ENVELOPE_BYTES_V1: usize = 47;
const ENDPOINT_GATE_WIRE_BYTES_V1: usize = ENDPOINT_ENVELOPE_BYTES_V1 + ENDPOINT_GATE_CORE_BYTES_V1;
const COEFFICIENT_PROOF_SET_BYTES_V1: usize = COEFFICIENT_IPAS_V1 * COEFFICIENT_CORE_BYTES_V1;
const TABLE_M_COMMITMENT_AND_CORE_BYTES_V1: usize =
    TABLE_M_COMMITMENT_BYTES_V1 + TABLE_M_CORE_BYTES_V1;
const MASK_COMMITMENT_AND_CORE_BYTES_V1: usize = MASK_COMMITMENT_BYTES_V1 + MASK_CORE_BYTES_V1;
const ENDPOINT_COMMITMENTS_AND_PROOF_BYTES_V1: usize =
    ENDPOINT_COMMITMENTS_BYTES_V1 + ENDPOINT_GATE_WIRE_BYTES_V1;
const COMMITTED_MLE_ACCOUNTED_WIRE_BYTES_V1: usize = COEFFICIENT_PROOF_SET_BYTES_V1
    + TABLE_M_COMMITMENT_AND_CORE_BYTES_V1
    + MASK_COMMITMENT_AND_CORE_BYTES_V1
    + ENDPOINT_COMMITMENTS_AND_PROOF_BYTES_V1;
const COMMITTED_MLE_PROFILE_ACCEPTED_V1: bool = false;
const COMMITTED_MLE_DATA_PLANE_WIRED_V1: bool = false;
const COMMITTED_MLE_PROVER_WIRED_V1: bool = false;
const COMMITTED_MLE_VERIFIER_WIRED_V1: bool = false;
const COMMITTED_MLE_PROOF_VERIFIED_V1: bool = false;
const COMMITTED_MLE_ZERO_KNOWLEDGE_ACCEPTED_V1: bool = false;
const COMMITTED_MLE_RECEIPT_ACCEPTED_V1: bool = false;
const COMMITTED_MLE_AUTHORITY_MINTED_V1: bool = false;
const COMMITTED_MLE_RSS_QUALIFIED_V1: bool = false;
const COMMITTED_MLE_RELEASE_READY_V1: bool = false;
const _: () = {
    assert!(LOOKUP_BITS_V1 == 29);
    assert!(COEFFICIENT_VECTOR_WIDTH_V1 == 16_384);
    assert!(TABLE_M_VECTOR_WIDTH_V1 == 32_768);
    assert!(MASK_VECTOR_WIDTH_V1 == 1_024);
    assert!(ENDPOINT_GATE_VECTOR_WIDTH_V1 == 32);
    assert!(COEFFICIENT_PROOF_SET_BYTES_V1 == 22_096);
    assert!(TABLE_M_COMMITMENT_AND_CORE_BYTES_V1 == 1_480);
    assert!(MASK_COMMITMENT_AND_CORE_BYTES_V1 == 1_150);
    assert!(ENDPOINT_COMMITMENTS_BYTES_V1 == 1_716);
    assert!(ENDPOINT_GATE_WIRE_BYTES_V1 == 834);
    assert!(ENDPOINT_COMMITMENTS_AND_PROOF_BYTES_V1 == 2_550);
    assert!(COMMITTED_MLE_ACCOUNTED_WIRE_BYTES_V1 == 27_276);
    assert!(!COMMITTED_MLE_RELEASE_READY_V1);
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommittedMleErrorV1 {
    Shape,
    Order,
    Context,
    PointEncoding,
    ScalarEncoding,
    WireEncoding,
    Arithmetic,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommittedMleProofRoleV1 {
    Coefficient(IpaStatementRoleV1),
    TableM,
    SumcheckMask,
    EndpointGates,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CommittedMleProofShapeV1 {
    ordinal: usize,
    role: CommittedMleProofRoleV1,
    width: usize,
    log_width: u8,
    vector_commitments: usize,
    scalar_commitments: usize,
    commitment_wire_bytes: usize,
    envelope_bytes: usize,
    core_bytes: usize,
}
impl CommittedMleProofShapeV1 {
    fn wire_bytes_v1(self) -> Result<usize, CommittedMleErrorV1> {
        self.commitment_wire_bytes
            .checked_add(self.envelope_bytes)
            .and_then(|bytes| bytes.checked_add(self.core_bytes))
            .ok_or(CommittedMleErrorV1::Arithmetic)
    }
}
fn proof_shape_v1(ordinal: usize) -> Result<CommittedMleProofShapeV1, CommittedMleErrorV1> {
    let (
        role,
        width,
        log_width,
        vector_commitments,
        scalar_commitments,
        commitment_wire_bytes,
        envelope_bytes,
        core_bytes,
    ) = match ordinal {
        0..=15 => (
            CommittedMleProofRoleV1::Coefficient(
                ipa_statement_role_v1(ordinal).map_err(|_| CommittedMleErrorV1::Shape)?,
            ),
            COEFFICIENT_VECTOR_WIDTH_V1,
            COORDINATE_BITS_V1,
            1,
            1,
            0,
            0,
            COEFFICIENT_CORE_BYTES_V1,
        ),
        16 => (
            CommittedMleProofRoleV1::TableM,
            TABLE_M_VECTOR_WIDTH_V1,
            PLANE_BITS_V1,
            1,
            1,
            TABLE_M_COMMITMENT_BYTES_V1,
            0,
            TABLE_M_CORE_BYTES_V1,
        ),
        17 => (
            CommittedMleProofRoleV1::SumcheckMask,
            MASK_VECTOR_WIDTH_V1,
            10,
            1,
            1,
            MASK_COMMITMENT_BYTES_V1,
            0,
            MASK_CORE_BYTES_V1,
        ),
        18 => (
            CommittedMleProofRoleV1::EndpointGates,
            ENDPOINT_GATE_VECTOR_WIDTH_V1,
            5,
            0,
            HIDDEN_ENDPOINTS_V1,
            ENDPOINT_COMMITMENTS_BYTES_V1,
            ENDPOINT_ENVELOPE_BYTES_V1,
            ENDPOINT_GATE_CORE_BYTES_V1,
        ),
        _ => return Err(CommittedMleErrorV1::Shape),
    };
    Ok(CommittedMleProofShapeV1 {
        ordinal,
        role,
        width,
        log_width,
        vector_commitments,
        scalar_commitments,
        commitment_wire_bytes,
        envelope_bytes,
        core_bytes,
    })
}
fn generalized_bp_core_bytes_v1(
    log_width: u8,
    vector_commitments: usize,
) -> Result<usize, CommittedMleErrorV1> {
    if vector_commitments > 1 {
        return Err(CommittedMleErrorV1::Shape);
    }
    let points = BP_FIXED_POINTS_C_LE_ONE_V1
        .checked_add(
            2_usize
                .checked_mul(usize::from(log_width))
                .ok_or(CommittedMleErrorV1::Arithmetic)?,
        )
        .ok_or(CommittedMleErrorV1::Arithmetic)?;
    points
        .checked_mul(POINT_BYTES_V1)
        .and_then(|bytes| bytes.checked_add(BP_FIXED_SCALARS_V1 * SCALAR_BYTES_V1))
        .ok_or(CommittedMleErrorV1::Arithmetic)
}
fn validate_static_profile_v1() -> Result<(), CommittedMleErrorV1> {
    if ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1 != PINNED_T256_BP_BASIS_DIGEST_V1
        || global_lookup_topology_digest_v1() != PINNED_TOPOLOGY_DIGEST_V1
        || challenge_manifest_digest_v1() != PINNED_CHALLENGE_MANIFEST_DIGEST_V1
        || COEFFICIENT_IPAS_V1 != 16
        || HIDDEN_ENDPOINTS_V1 != 52
        || ENDPOINT_STATEMENTS_V1 != 16
        || ENDPOINT_GATES_V1 != 32
    {
        return Err(CommittedMleErrorV1::Context);
    }
    let mut accounted = 0_usize;
    for ordinal in 0..COMMITTED_MLE_PROOFS_V1 {
        let shape = proof_shape_v1(ordinal)?;
        if shape.width != 1_usize << shape.log_width
            || shape.core_bytes
                != generalized_bp_core_bytes_v1(shape.log_width, shape.vector_commitments)?
        {
            return Err(CommittedMleErrorV1::Shape);
        }
        if let CommittedMleProofRoleV1::Coefficient(role) = shape.role
            && ipa_tag_v1(role) != ordinal as u8 + 1
        {
            return Err(CommittedMleErrorV1::Order);
        }
        accounted = accounted
            .checked_add(shape.wire_bytes_v1()?)
            .ok_or(CommittedMleErrorV1::Arithmetic)?;
    }
    if accounted != COMMITTED_MLE_ACCOUNTED_WIRE_BYTES_V1 {
        return Err(CommittedMleErrorV1::Shape);
    }
    Ok(())
}
fn absorb_len_prefixed_v1(hash: &mut Keccak256, bytes: &[u8]) -> Result<(), CommittedMleErrorV1> {
    let len = u16::try_from(bytes.len()).map_err(|_| CommittedMleErrorV1::Arithmetic)?;
    hash.update(&len.to_be_bytes());
    hash.update(bytes);
    Ok(())
}
fn role_tag_v1(role: CommittedMleProofRoleV1) -> u8 {
    match role {
        CommittedMleProofRoleV1::Coefficient(role) => ipa_tag_v1(role),
        CommittedMleProofRoleV1::TableM => 17,
        CommittedMleProofRoleV1::SumcheckMask => 18,
        CommittedMleProofRoleV1::EndpointGates => 19,
    }
}
fn committed_mle_transcript_manifest_digest_v1() -> Result<[u8; 32], CommittedMleErrorV1> {
    validate_static_profile_v1()?;
    let mut hash = Keccak256::new();
    hash.update(COMMITTED_MLE_MANIFEST_DOMAIN_V1);
    hash.update(&[COMMITTED_MLE_VERSION_V1]);
    hash.update(&PINNED_TOPOLOGY_DIGEST_V1);
    hash.update(&PINNED_CHALLENGE_MANIFEST_DIGEST_V1);
    hash.update(&PINNED_T256_BP_BASIS_DIGEST_V1);
    hash.update(&[COORDINATE_BITS_V1, PLANE_BITS_V1, LOOKUP_BITS_V1]);
    absorb_len_prefixed_v1(&mut hash, COMMITTED_MLE_LAYOUT_LANGUAGE_V1)?;
    absorb_len_prefixed_v1(&mut hash, COMMITTED_MLE_TRANSCRIPT_LANGUAGE_V1)?;
    absorb_len_prefixed_v1(&mut hash, COMMITTED_MLE_BP_PRIMITIVE_LANGUAGE_V1)?;
    absorb_len_prefixed_v1(&mut hash, COMMITTED_MLE_OPENING_DIGEST_LANGUAGE_V1)?;
    absorb_len_prefixed_v1(&mut hash, COMMITTED_MLE_ACCOUNTING_SCOPE_LANGUAGE_V1)?;
    absorb_len_prefixed_v1(&mut hash, ENDPOINT_ENVELOPE_LANGUAGE_V1)?;
    hash.update(&(COMMITTED_MLE_PROOFS_V1 as u16).to_be_bytes());
    for ordinal in 0..COMMITTED_MLE_PROOFS_V1 {
        let shape = proof_shape_v1(ordinal)?;
        hash.update(&(shape.ordinal as u16).to_be_bytes());
        hash.update(&[role_tag_v1(shape.role), shape.log_width]);
        hash.update(&(shape.width as u32).to_be_bytes());
        hash.update(&(shape.vector_commitments as u16).to_be_bytes());
        hash.update(&(shape.scalar_commitments as u16).to_be_bytes());
        hash.update(&(shape.commitment_wire_bytes as u32).to_be_bytes());
        hash.update(&(shape.envelope_bytes as u16).to_be_bytes());
        hash.update(&(shape.core_bytes as u16).to_be_bytes());
        hash.update(&(shape.wire_bytes_v1()? as u32).to_be_bytes());
    }
    hash.update(&(COMMITTED_MLE_ACCOUNTED_WIRE_BYTES_V1 as u32).to_be_bytes());
    hash.update(&[
        COMMITTED_MLE_PROFILE_ACCEPTED_V1 as u8,
        COMMITTED_MLE_DATA_PLANE_WIRED_V1 as u8,
        COMMITTED_MLE_PROVER_WIRED_V1 as u8,
        COMMITTED_MLE_VERIFIER_WIRED_V1 as u8,
        COMMITTED_MLE_PROOF_VERIFIED_V1 as u8,
        COMMITTED_MLE_ZERO_KNOWLEDGE_ACCEPTED_V1 as u8,
        COMMITTED_MLE_RECEIPT_ACCEPTED_V1 as u8,
        COMMITTED_MLE_AUTHORITY_MINTED_V1 as u8,
        COMMITTED_MLE_RSS_QUALIFIED_V1 as u8,
        COMMITTED_MLE_RELEASE_READY_V1 as u8,
    ]);
    let digest = hash.finalize();
    (digest != [0; 32])
        .then_some(digest)
        .ok_or(CommittedMleErrorV1::Context)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GlobalLookupCommittedMleProfileV1 {
    topology_digest: [u8; 32],
    challenge_manifest_digest: [u8; 32],
    basis_digest: [u8; 32],
    purpose_manifest_digest: [u8; 32],
    proof_count: usize,
    accounted_wire_bytes: usize,
}
fn global_lookup_committed_mle_profile_v1()
-> Result<GlobalLookupCommittedMleProfileV1, CommittedMleErrorV1> {
    validate_static_profile_v1()?;
    Ok(GlobalLookupCommittedMleProfileV1 {
        topology_digest: PINNED_TOPOLOGY_DIGEST_V1,
        challenge_manifest_digest: PINNED_CHALLENGE_MANIFEST_DIGEST_V1,
        basis_digest: PINNED_T256_BP_BASIS_DIGEST_V1,
        purpose_manifest_digest: committed_mle_transcript_manifest_digest_v1()?,
        proof_count: COMMITTED_MLE_PROOFS_V1,
        accounted_wire_bytes: COMMITTED_MLE_ACCOUNTED_WIRE_BYTES_V1,
    })
}
enum CommittedMleDataPlaneSealV1 {
    Production {
        authenticated_sumcheck_columns: Infallible,
        retained_vector_openings: Infallible,
        retained_scalar_endpoints: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
enum CommittedMleProofSessionSealV1 {
    Production {
        outer_opening_stage: Infallible,
        purpose_bound_randomness: Infallible,
        ordered_proof_sink: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
#[must_use = "dropping this move-only owner closes the unconsumed committed-MLE prerequisite"]
struct GlobalLookupCommittedMleOwnerV1 {
    data_plane: CommittedMleDataPlaneSealV1,
    proof_session: CommittedMleProofSessionSealV1,
}
struct BorrowedPointV1<'a>(&'a [u8; POINT_BYTES_V1]);
fn borrow_point_exact_v1(bytes: &[u8]) -> Result<BorrowedPointV1<'_>, CommittedMleErrorV1> {
    let fixed: &[u8; POINT_BYTES_V1] = bytes
        .try_into()
        .map_err(|_| CommittedMleErrorV1::WireEncoding)?;
    Point::from_non_identity_wire_bytes_exact(fixed)
        .map_err(|_| CommittedMleErrorV1::PointEncoding)?;
    Ok(BorrowedPointV1(fixed))
}
enum BorrowedBpCoreV1<'a> {
    N1024(&'a [u8; MASK_CORE_BYTES_V1]),
    N16384(&'a [u8; COEFFICIENT_CORE_BYTES_V1]),
    N32768(&'a [u8; TABLE_M_CORE_BYTES_V1]),
    EndpointN32(&'a [u8; ENDPOINT_GATE_CORE_BYTES_V1]),
}
impl BorrowedBpCoreV1<'_> {
    fn bytes_v1(&self) -> &[u8] {
        match self {
            Self::N1024(bytes) => *bytes,
            Self::N16384(bytes) => *bytes,
            Self::N32768(bytes) => *bytes,
            Self::EndpointN32(bytes) => *bytes,
        }
    }
}
fn take_point_v1(bytes: &[u8], cursor: &mut usize) -> Result<(), CommittedMleErrorV1> {
    let end = cursor
        .checked_add(POINT_BYTES_V1)
        .ok_or(CommittedMleErrorV1::Arithmetic)?;
    borrow_point_exact_v1(
        bytes
            .get(*cursor..end)
            .ok_or(CommittedMleErrorV1::WireEncoding)?,
    )?;
    *cursor = end;
    Ok(())
}
fn take_scalar_v1(bytes: &[u8], cursor: &mut usize) -> Result<(), CommittedMleErrorV1> {
    let end = cursor
        .checked_add(SCALAR_BYTES_V1)
        .ok_or(CommittedMleErrorV1::Arithmetic)?;
    let encoded: [u8; SCALAR_BYTES_V1] = bytes
        .get(*cursor..end)
        .ok_or(CommittedMleErrorV1::WireEncoding)?
        .try_into()
        .map_err(|_| CommittedMleErrorV1::WireEncoding)?;
    Scalar::from_le_bytes_exact(encoded).map_err(|_| CommittedMleErrorV1::ScalarEncoding)?;
    *cursor = end;
    Ok(())
}
fn validate_bp_core_v1(bytes: &[u8], log_width: u8) -> Result<(), CommittedMleErrorV1> {
    let expected = generalized_bp_core_bytes_v1(log_width, 1)?;
    if bytes.len() != expected {
        return Err(CommittedMleErrorV1::WireEncoding);
    }
    let mut cursor = 0;
    for _ in 0..BP_FIXED_POINTS_C_LE_ONE_V1 {
        take_point_v1(bytes, &mut cursor)?;
    }
    for _ in 0..3 {
        take_scalar_v1(bytes, &mut cursor)?;
    }
    for _ in 0..2 * usize::from(log_width) {
        take_point_v1(bytes, &mut cursor)?;
    }
    for _ in 0..2 {
        take_scalar_v1(bytes, &mut cursor)?;
    }
    (cursor == bytes.len())
        .then_some(())
        .ok_or(CommittedMleErrorV1::WireEncoding)
}
fn borrow_bp_core_exact_v1(
    log_width: u8,
    bytes: &[u8],
) -> Result<BorrowedBpCoreV1<'_>, CommittedMleErrorV1> {
    validate_bp_core_v1(bytes, log_width)?;
    match log_width {
        10 => Ok(BorrowedBpCoreV1::N1024(
            bytes
                .try_into()
                .map_err(|_| CommittedMleErrorV1::WireEncoding)?,
        )),
        14 => Ok(BorrowedBpCoreV1::N16384(
            bytes
                .try_into()
                .map_err(|_| CommittedMleErrorV1::WireEncoding)?,
        )),
        15 => Ok(BorrowedBpCoreV1::N32768(
            bytes
                .try_into()
                .map_err(|_| CommittedMleErrorV1::WireEncoding)?,
        )),
        5 => Ok(BorrowedBpCoreV1::EndpointN32(
            bytes
                .try_into()
                .map_err(|_| CommittedMleErrorV1::WireEncoding)?,
        )),
        _ => Err(CommittedMleErrorV1::Shape),
    }
}
struct BorrowedCoefficientOpeningV1<'a> {
    statement_ordinal: u8,
    core: BorrowedBpCoreV1<'a>,
}
fn borrow_coefficient_opening_exact_v1(
    statement_ordinal: usize,
    core: &[u8],
) -> Result<BorrowedCoefficientOpeningV1<'_>, CommittedMleErrorV1> {
    if statement_ordinal >= COEFFICIENT_IPAS_V1 {
        return Err(CommittedMleErrorV1::Order);
    }
    proof_shape_v1(statement_ordinal)?;
    Ok(BorrowedCoefficientOpeningV1 {
        statement_ordinal: statement_ordinal as u8,
        core: borrow_bp_core_exact_v1(COORDINATE_BITS_V1, core)?,
    })
}
struct BorrowedTableMOpeningV1<'a> {
    commitment: BorrowedPointV1<'a>,
    core: BorrowedBpCoreV1<'a>,
}
fn borrow_table_m_opening_exact_v1<'a>(
    commitment: &'a [u8],
    core: &'a [u8],
) -> Result<BorrowedTableMOpeningV1<'a>, CommittedMleErrorV1> {
    Ok(BorrowedTableMOpeningV1 {
        commitment: borrow_point_exact_v1(commitment)?,
        core: borrow_bp_core_exact_v1(PLANE_BITS_V1, core)?,
    })
}
struct BorrowedMaskOpeningV1<'a> {
    commitment: BorrowedPointV1<'a>,
    core: BorrowedBpCoreV1<'a>,
}
fn borrow_mask_opening_exact_v1<'a>(
    commitment: &'a [u8],
    core: &'a [u8],
) -> Result<BorrowedMaskOpeningV1<'a>, CommittedMleErrorV1> {
    Ok(BorrowedMaskOpeningV1 {
        commitment: borrow_point_exact_v1(commitment)?,
        core: borrow_bp_core_exact_v1(10, core)?,
    })
}
const ENDPOINT_ENVELOPE_MAGIC_V1: [u8; 4] = *b"ZGEP";
const ENDPOINT_ENVELOPE_FLAGS_V1: u8 = 0;
fn write_endpoint_envelope_v1(
    output: &mut [u8; ENDPOINT_ENVELOPE_BYTES_V1],
) -> Result<(), CommittedMleErrorV1> {
    output.fill(0);
    output[..4].copy_from_slice(&ENDPOINT_ENVELOPE_MAGIC_V1);
    output[4] = COMMITTED_MLE_VERSION_V1;
    output[5] = ENDPOINT_ENVELOPE_FLAGS_V1;
    output[6] = 0;
    output[7] = 5;
    output[8] = ENDPOINT_STATEMENTS_V1 as u8;
    output[9..11].copy_from_slice(&(ENDPOINT_GATES_V1 as u16).to_be_bytes());
    output[11..13].copy_from_slice(&(HIDDEN_ENDPOINTS_V1 as u16).to_be_bytes());
    output[13..15].copy_from_slice(&(ENDPOINT_GATE_CORE_BYTES_V1 as u16).to_be_bytes());
    output[15..].copy_from_slice(&committed_mle_transcript_manifest_digest_v1()?);
    Ok(())
}
struct BorrowedEndpointEnvelopeV1<'a>(&'a [u8; ENDPOINT_ENVELOPE_BYTES_V1]);
fn borrow_endpoint_envelope_exact_v1(
    bytes: &[u8],
) -> Result<BorrowedEndpointEnvelopeV1<'_>, CommittedMleErrorV1> {
    let fixed: &[u8; ENDPOINT_ENVELOPE_BYTES_V1] = bytes
        .try_into()
        .map_err(|_| CommittedMleErrorV1::WireEncoding)?;
    let mut expected = [0_u8; ENDPOINT_ENVELOPE_BYTES_V1];
    write_endpoint_envelope_v1(&mut expected)?;
    if *fixed != expected {
        return Err(CommittedMleErrorV1::WireEncoding);
    }
    Ok(BorrowedEndpointEnvelopeV1(fixed))
}
struct BorrowedEndpointGateOpeningV1<'a> {
    endpoint_commitments: &'a [u8; ENDPOINT_COMMITMENTS_BYTES_V1],
    envelope: BorrowedEndpointEnvelopeV1<'a>,
    core: BorrowedBpCoreV1<'a>,
}
fn borrow_endpoint_gate_opening_exact_v1<'a>(
    endpoint_commitments: &'a [u8],
    envelope: &'a [u8],
    core: &'a [u8],
) -> Result<BorrowedEndpointGateOpeningV1<'a>, CommittedMleErrorV1> {
    let fixed_commitments: &[u8; ENDPOINT_COMMITMENTS_BYTES_V1] =
        endpoint_commitments
            .try_into()
            .map_err(|_| CommittedMleErrorV1::WireEncoding)?;
    for encoded in fixed_commitments.chunks_exact(POINT_BYTES_V1) {
        borrow_point_exact_v1(encoded)?;
    }
    Ok(BorrowedEndpointGateOpeningV1 {
        endpoint_commitments: fixed_commitments,
        envelope: borrow_endpoint_envelope_exact_v1(envelope)?,
        core: borrow_bp_core_exact_v1(5, core)?,
    })
}
#[cfg(test)]
#[path = "global_lookup_committed_mle_v1_tests.rs"]
mod tests;
