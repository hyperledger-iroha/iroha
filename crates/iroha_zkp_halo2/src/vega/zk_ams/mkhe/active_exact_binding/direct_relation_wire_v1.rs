//! Candidate-only canonical wire for the six-witness direct relations.
//!
//! This module freezes statement/body framing and structural predecode, and
//! privately replays the candidate RKG-round-one and Galois proof equations
//! plus typed object authentication. It deliberately supplies no admission or
//! release receipt, no public verification result, and no release transition.
use super::super::{
    ZkAmsMkheErrorV1,
    direct_collective_eval_ceremony::ZkAmsMkheDirectCeremonyRoundV1,
    direct_object_transport::ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1,
    exact_eight_chunk_membership::{
        ExactEightChunkMembershipEvidenceV1, ExactEightChunkMembershipRoleV1,
        PreflightedExactEightChunkMembershipWireV1,
    },
};
use super::{
    CHALLENGE_REPETITIONS_V1, ExactBindingTranscriptContextV1, PersistentDirectRelationV1,
};
use crate::vega::sponge::Keccak256;
#[path = "direct_relation_wire_v1/predecode_v1.rs"]
mod predecode_v1;
#[allow(
    unused_imports,
    reason = "candidate-only semantic verifier seam is retained for the pending direct-relation owner and cannot mint admission or release authority"
)]
pub(super) use predecode_v1::{
    CompletedDirectGaloisSemanticVerificationV1, CompletedDirectRkgOneSemanticVerificationV1,
    verify_direct_galois_semantic_candidate_v1, verify_direct_rkg_one_semantic_candidate_v1,
};
#[path = "direct_relation_wire_v1/response_commitment_v1.rs"]
pub(super) mod response_commitment_v1;
#[path = "direct_relation_wire_v1/rkg_one_creator_membership_v1.rs"]
mod rkg_one_creator_membership_v1;
#[path = "direct_relation_wire_v1/rkg_one_creator_prover_v1.rs"]
mod rkg_one_creator_prover_v1;
#[path = "direct_relation_wire_v1/rkg_one_creator_response_v1.rs"]
mod rkg_one_creator_response_v1;
#[path = "direct_relation_wire_v1/statement_v1.rs"]
pub(super) mod statement_v1;
pub(in crate::vega::zk_ams::mkhe) use rkg_one_creator_prover_v1::{
    PublishedDirectRkgOneProofOwnerV2, SealedDirectRkgOneProofOwnerV1,
    seal_direct_rkg_one_proof_owner_v1,
};
pub(super) use statement_v1::ExpectedDirectRelationStatementV1;
#[cfg(test)]
#[allow(
    unused_imports,
    reason = "test namespace preserves every reviewed direct-relation object role while only three are exercised by current cases"
)]
pub(super) use statement_v1::{
    AggregateH0ObjectRoleV1, AggregateH1ObjectRoleV1, GaloisBObjectRoleV1, RkgKObjectRoleV1,
    RkgNormalizationObjectRoleV1,
};
pub(in crate::vega::zk_ams::mkhe) use statement_v1::{
    DirectPolynomialObjectV1, DirectRelationPublicObjectsV1, PreparedDirectRkgOneStatementCoreV1,
    RkgH0ObjectRoleV1, RkgH1ObjectRoleV1,
};
#[cfg(test)]
#[path = "direct_relation_wire_v1/kats.rs"]
mod kats;
#[cfg(test)]
#[path = "direct_relation_wire_v1/tests.rs"]
mod tests;
const DIRECT_RELATION_WIRE_MAGIC_V1: [u8; 4] = *b"ZAXR";
const DIRECT_RELATION_STATEMENT_MAGIC_V1: [u8; 4] = *b"ZADS";
pub(super) const DIRECT_RELATION_CODEC_VERSION_V1: u8 = 1;
const HEADER_BYTES_V1: usize = 80;
const STATEMENT_PREFIX_BYTES_V1: usize = 544;
const OBJECT_ENTRY_BYTES_V1: usize = 32 + ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1;
const STATEMENT_TRAILER_BYTES_V1: usize = 64;
const MAX_STATEMENT_BYTES_V1: usize = 938;
const RKG_ONE_STATEMENT_BYTES_V1: usize = 828;
const RKG_TWO_STATEMENT_BYTES_V1: usize = 938;
const NORMALIZE_STATEMENT_BYTES_V1: usize = 828;
const GALOIS_STATEMENT_BYTES_V1: usize = 718;
const WITNESS_COUNT_V1: usize = 6;
const BOUND_ONE_WITNESS_COUNT_V1: usize = 2;
const BOUND_TWO_WITNESS_COUNT_V1: usize = 4;
const CHUNKS_PER_WITNESS_V1: usize = 8;
const DIRECT_BOUND_ONE_MEMBERSHIP_BYTES_V1: usize = 12_291;
const DIRECT_BOUND_TWO_MEMBERSHIP_BYTES_V1: usize = 12_819;
const MEMBERSHIP_BYTES_V1: usize =
    2 * DIRECT_BOUND_ONE_MEMBERSHIP_BYTES_V1 + 4 * DIRECT_BOUND_TWO_MEMBERSHIP_BYTES_V1;
const RESPONSE_BYTES_V1: usize = 25_165_824;
const BLIND_RESPONSE_BYTES_V1: usize = 6_144;
const CHALLENGE_SEED_BYTES_V1: usize = 32;
const BODY_BYTES_V1: usize =
    MEMBERSHIP_BYTES_V1 + RESPONSE_BYTES_V1 + BLIND_RESPONSE_BYTES_V1 + CHALLENGE_SEED_BYTES_V1;
const EXACT_POLYNOMIAL_OBJECT_BYTES_V1: u64 = 39_845_888;
const RELEASE_RNS_LIMBS_V1: usize = 38;
const RELEASE_RING_COEFFICIENTS_V1: usize = 131_072;
const RECONSTRUCTED_COMMITMENT_POINTS_V1: usize = WITNESS_COUNT_V1 * CHUNKS_PER_WITNESS_V1;
const RECONSTRUCTED_COMMITMENT_BYTES_V1: usize = RECONSTRUCTED_COMMITMENT_POINTS_V1 * 33;
const RELATION_CORE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-relation-wire.statement-core";
const FINAL_STATEMENT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-relation-wire.final-statement";
const MEMBERSHIP_SLOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-relation-wire.membership-slot";
const ORDERED_COMMITMENT_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-relation-wire.ordered-commitment-root";
const ORDERED_MEMBERSHIP_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-relation-wire.ordered-membership-root";
pub(super) const RELATION_LINEAGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-relation-wire.lineage";
const RNS_FIRST_MESSAGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-relation-wire.rns-first-message";
const COMMITMENT_FIRST_MESSAGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-relation-wire.commitment-first-message";
const MEMBERSHIP_FRAME_OFFSETS_V1: [usize; WITNESS_COUNT_V1] =
    [0, 12_291, 24_582, 37_401, 50_220, 63_039];
const _: () = {
    assert!(OBJECT_ENTRY_BYTES_V1 == 110);
    assert!(
        RKG_ONE_STATEMENT_BYTES_V1
            == STATEMENT_PREFIX_BYTES_V1 + 2 * 110 + STATEMENT_TRAILER_BYTES_V1
    );
    assert!(
        RKG_TWO_STATEMENT_BYTES_V1
            == STATEMENT_PREFIX_BYTES_V1 + 3 * 110 + STATEMENT_TRAILER_BYTES_V1
    );
    assert!(NORMALIZE_STATEMENT_BYTES_V1 == RKG_ONE_STATEMENT_BYTES_V1);
    assert!(
        GALOIS_STATEMENT_BYTES_V1 == STATEMENT_PREFIX_BYTES_V1 + 110 + STATEMENT_TRAILER_BYTES_V1
    );
    assert!(MEMBERSHIP_BYTES_V1 == 75_858);
    assert!(BODY_BYTES_V1 == 25_247_858);
    assert!(HEADER_BYTES_V1 + RKG_ONE_STATEMENT_BYTES_V1 + BODY_BYTES_V1 == 25_248_766);
    assert!(HEADER_BYTES_V1 + RKG_TWO_STATEMENT_BYTES_V1 + BODY_BYTES_V1 == 25_248_876);
    assert!(HEADER_BYTES_V1 + GALOIS_STATEMENT_BYTES_V1 + BODY_BYTES_V1 == 25_248_656);
    assert!(2 * EXACT_POLYNOMIAL_OBJECT_BYTES_V1 > 64 * 1024 * 1024);
};
impl PersistentDirectRelationV1 {
    const fn statement_bytes(self) -> usize {
        match self {
            Self::RkgRoundOne => RKG_ONE_STATEMENT_BYTES_V1,
            Self::RkgRoundTwo => RKG_TWO_STATEMENT_BYTES_V1,
            Self::RkgNormalize => NORMALIZE_STATEMENT_BYTES_V1,
            Self::Galois => GALOIS_STATEMENT_BYTES_V1,
        }
    }
    const fn object_count(self) -> usize {
        match self {
            Self::RkgRoundOne | Self::RkgNormalize => 2,
            Self::RkgRoundTwo => 3,
            Self::Galois => 1,
        }
    }
    const fn active_witness_mask(self) -> u8 {
        match self {
            Self::RkgRoundOne => 0x0f,
            Self::RkgRoundTwo => 0x13,
            Self::RkgNormalize => 0x21,
            Self::Galois => 0x05,
        }
    }
    const fn forced_zero_witness_mask(self) -> u8 {
        match self {
            Self::RkgRoundOne => 0x30,
            Self::RkgRoundTwo => 0x2c,
            Self::RkgNormalize => 0x1e,
            Self::Galois => 0x3a,
        }
    }
    const fn ceremony_round(self) -> ZkAmsMkheDirectCeremonyRoundV1 {
        match self {
            Self::RkgRoundOne => ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne,
            Self::RkgRoundTwo => ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo,
            Self::RkgNormalize => ZkAmsMkheDirectCeremonyRoundV1::RkgNormalize,
            Self::Galois => ZkAmsMkheDirectCeremonyRoundV1::Galois,
        }
    }
    const fn rns_row_tags(self) -> ([u8; 5], usize) {
        const ZERO: u8 = 0x80;
        match self {
            Self::RkgRoundOne => ([1, 2, ZERO | 4, ZERO | 5, 0], 4),
            Self::RkgRoundTwo => ([3, ZERO | 2, ZERO | 3, ZERO | 5, 0], 4),
            Self::RkgNormalize => ([4, ZERO | 1, ZERO | 2, ZERO | 3, ZERO | 4], 5),
            Self::Galois => ([5, ZERO | 1, ZERO | 3, ZERO | 4, ZERO | 5], 5),
        }
    }
}
fn membership_share_statement_digest(
    relation: PersistentDirectRelationV1,
    core_digest: [u8; 32],
    slot: usize,
) -> [u8; 32] {
    let active = relation.active_witness_mask() & (1 << slot) != 0;
    let forced_zero = relation.forced_zero_witness_mask() & (1 << slot) != 0;
    let mut hash = Keccak256::new();
    hash.update(MEMBERSHIP_SLOT_DOMAIN_V1);
    hash.update(&[DIRECT_RELATION_CODEC_VERSION_V1, relation as u8, slot as u8]);
    hash.update(&core_digest);
    hash.update(&[
        if slot < 2 { 1 } else { 2 },
        u8::from(active),
        u8::from(forced_zero),
    ]);
    hash.finalize()
}
fn canonical_header(expected: &ExpectedDirectRelationStatementV1) -> [u8; HEADER_BYTES_V1] {
    canonical_header_fields_v1(
        expected.relation(),
        expected.bytes().len(),
        expected.statement_digest(),
    )
}
fn canonical_header_fields_v1(
    relation: PersistentDirectRelationV1,
    statement_bytes: usize,
    statement_digest: [u8; 32],
) -> [u8; HEADER_BYTES_V1] {
    let total = HEADER_BYTES_V1 + statement_bytes + BODY_BYTES_V1;
    let mut bytes = [0_u8; HEADER_BYTES_V1];
    bytes[..4].copy_from_slice(&DIRECT_RELATION_WIRE_MAGIC_V1);
    bytes[4] = DIRECT_RELATION_CODEC_VERSION_V1;
    bytes[5] = relation as u8;
    bytes[6] = WITNESS_COUNT_V1 as u8;
    bytes[7] = CHALLENGE_REPETITIONS_V1 as u8;
    bytes[8] = CHUNKS_PER_WITNESS_V1 as u8;
    bytes[9] = BOUND_ONE_WITNESS_COUNT_V1 as u8;
    bytes[10] = BOUND_TWO_WITNESS_COUNT_V1 as u8;
    bytes[11] = relation.object_count() as u8;
    for (offset, value) in [
        (12, HEADER_BYTES_V1),
        (16, statement_bytes),
        (20, MEMBERSHIP_BYTES_V1),
        (24, RESPONSE_BYTES_V1),
        (28, BLIND_RESPONSE_BYTES_V1),
        (32, CHALLENGE_SEED_BYTES_V1),
        (36, BODY_BYTES_V1),
        (40, total),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&(value as u32).to_be_bytes());
    }
    bytes[48..80].copy_from_slice(&statement_digest);
    bytes
}
trait DirectRelationMembershipSummaryV1 {
    fn commitment_set_digest(&self) -> [u8; 32];
    fn proof_set_digest(&self) -> [u8; 32];
    fn verifier_transcript_digest(&self) -> [u8; 32];
}
impl<R: ExactEightChunkMembershipRoleV1> DirectRelationMembershipSummaryV1
    for ExactEightChunkMembershipEvidenceV1<R>
{
    fn commitment_set_digest(&self) -> [u8; 32] {
        self.commitment_set_digest()
    }
    fn proof_set_digest(&self) -> [u8; 32] {
        self.proof_set_digest()
    }
    fn verifier_transcript_digest(&self) -> [u8; 32] {
        self.verifier_transcript_digest()
    }
}
impl<R: ExactEightChunkMembershipRoleV1> DirectRelationMembershipSummaryV1
    for PreflightedExactEightChunkMembershipWireV1<'_, R>
{
    fn commitment_set_digest(&self) -> [u8; 32] {
        self.commitment_set_digest()
    }
    fn proof_set_digest(&self) -> [u8; 32] {
        self.proof_set_digest()
    }
    fn verifier_transcript_digest(&self) -> [u8; 32] {
        self.verifier_transcript_digest()
    }
}
fn ordered_membership_roots<BoundOne, BoundTwo>(
    relation: PersistentDirectRelationV1,
    core_digest: [u8; 32],
    bound_one: &[BoundOne; 2],
    bound_two: &[BoundTwo; 4],
) -> ([u8; 32], [u8; 32])
where
    BoundOne: DirectRelationMembershipSummaryV1,
    BoundTwo: DirectRelationMembershipSummaryV1,
{
    let mut commitment = Keccak256::new();
    commitment.update(ORDERED_COMMITMENT_ROOT_DOMAIN_V1);
    commitment.update(&[DIRECT_RELATION_CODEC_VERSION_V1, relation as u8, 6]);
    commitment.update(&core_digest);
    let mut membership = Keccak256::new();
    membership.update(ORDERED_MEMBERSHIP_ROOT_DOMAIN_V1);
    membership.update(&[DIRECT_RELATION_CODEC_VERSION_V1, relation as u8, 6]);
    membership.update(&core_digest);
    for (slot, evidence) in bound_one.iter().enumerate() {
        commitment.update(&[slot as u8]);
        commitment.update(&evidence.commitment_set_digest());
        membership.update(&[slot as u8]);
        membership.update(&evidence.proof_set_digest());
        membership.update(&evidence.verifier_transcript_digest());
    }
    for (index, evidence) in bound_two.iter().enumerate() {
        let slot = index + 2;
        commitment.update(&[slot as u8]);
        commitment.update(&evidence.commitment_set_digest());
        membership.update(&[slot as u8]);
        membership.update(&evidence.proof_set_digest());
        membership.update(&evidence.verifier_transcript_digest());
    }
    (commitment.finalize(), membership.finalize())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DirectRelationFirstMessageDigestsV1 {
    rns: [[u8; 32]; CHALLENGE_REPETITIONS_V1],
    commitments: [[u8; 32]; CHALLENGE_REPETITIONS_V1],
}
impl DirectRelationFirstMessageDigestsV1 {
    fn new(
        rns: [[u8; 32]; CHALLENGE_REPETITIONS_V1],
        commitments: [[u8; 32]; CHALLENGE_REPETITIONS_V1],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if rns
            .iter()
            .chain(commitments.iter())
            .any(|digest| *digest == [0; 32])
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(Self { rns, commitments })
    }
}
struct DirectRelationRnsFirstMessageHasherV1 {
    hash: Keccak256,
    relation: PersistentDirectRelationV1,
    next_row: usize,
    next_limb: usize,
}
impl DirectRelationRnsFirstMessageHasherV1 {
    fn new(relation: PersistentDirectRelationV1) -> Self {
        let (rows, count) = relation.rns_row_tags();
        let mut hash = Keccak256::new();
        hash.update(RNS_FIRST_MESSAGE_DOMAIN_V1);
        hash.update(&[
            DIRECT_RELATION_CODEC_VERSION_V1,
            relation as u8,
            count as u8,
        ]);
        hash.update(&rows[..count]);
        hash.update(&(RELEASE_RNS_LIMBS_V1 as u16).to_be_bytes());
        hash.update(&(RELEASE_RING_COEFFICIENTS_V1 as u32).to_be_bytes());
        Self {
            hash,
            relation,
            next_row: 0,
            next_limb: 0,
        }
    }
    fn absorb_limb(
        &mut self,
        row: usize,
        limb: usize,
        canonical_residue_bytes: &[u8],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let (_, row_count) = self.relation.rns_row_tags();
        if row != self.next_row
            || limb != self.next_limb
            || row >= row_count
            || canonical_residue_bytes.len() != RELEASE_RING_COEFFICIENTS_V1 * 8
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        self.hash.update(&[row as u8, limb as u8]);
        self.hash.update(canonical_residue_bytes);
        self.next_limb += 1;
        if self.next_limb == RELEASE_RNS_LIMBS_V1 {
            self.next_limb = 0;
            self.next_row += 1;
        }
        Ok(())
    }
    fn finish(self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let (_, row_count) = self.relation.rns_row_tags();
        if self.next_row != row_count || self.next_limb != 0 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(self.hash.finalize())
    }
}
fn commitment_first_message_digest(
    relation: PersistentDirectRelationV1,
    bytes: &[u8],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if bytes.len() != RECONSTRUCTED_COMMITMENT_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let mut hash = Keccak256::new();
    hash.update(COMMITMENT_FIRST_MESSAGE_DOMAIN_V1);
    hash.update(&[
        DIRECT_RELATION_CODEC_VERSION_V1,
        relation as u8,
        WITNESS_COUNT_V1 as u8,
        CHUNKS_PER_WITNESS_V1 as u8,
    ]);
    hash.update(bytes);
    Ok(hash.finalize())
}
fn challenge_vector_from_first_messages(
    context: ExactBindingTranscriptContextV1,
    first_messages: DirectRelationFirstMessageDigestsV1,
) -> Result<([u8; 32], [u32; CHALLENGE_REPETITIONS_V1]), ZkAmsMkheErrorV1> {
    super::challenge_vector(context, first_messages.rns, first_messages.commitments)
}
