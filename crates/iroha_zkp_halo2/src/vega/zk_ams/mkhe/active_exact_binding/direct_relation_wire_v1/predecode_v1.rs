#[cfg(test)]
use super::super::super::exact_eight_chunk_membership::{
    ExactEightChunkMembershipErrorV1, ExactEightChunkMembershipEvidenceV1,
};
use super::super::super::{
    ZkAmsMkheErrorV1,
    direct_collective_eval_ceremony::ZkAmsMkheDirectCeremonyContextV1,
    exact_eight_chunk_membership::{
        DirectRelationBoundOneMembershipRoleV1, DirectRelationBoundTwoMembershipRoleV1,
        ExactEightChunkMembershipContextV1, ExactEightChunkMembershipRoleV1,
        PreflightedExactEightChunkMembershipWireV1,
    },
    wire::ZK_AMS_MKHE_MAX_PROOF_BYTES_V1,
};
use super::super::{
    ExactBindingTranscriptContextV1, PersistentDirectRelationV1, RESPONSE_COEFFICIENT_BOUND_V1,
    VerifiedPersistentWitnessDirectRelationUseV1,
};
use super::statement_v1::ExpectedDirectRelationStatementV1;
use super::{
    BLIND_RESPONSE_BYTES_V1, BODY_BYTES_V1, CHALLENGE_SEED_BYTES_V1, CHUNKS_PER_WITNESS_V1,
    DIRECT_BOUND_ONE_MEMBERSHIP_BYTES_V1, DIRECT_BOUND_TWO_MEMBERSHIP_BYTES_V1,
    DirectRelationFirstMessageDigestsV1, DirectRelationPublicObjectsV1, HEADER_BYTES_V1,
    MEMBERSHIP_BYTES_V1, MEMBERSHIP_FRAME_OFFSETS_V1, RESPONSE_BYTES_V1, WITNESS_COUNT_V1,
    canonical_header, challenge_vector_from_first_messages, membership_share_statement_digest,
    ordered_membership_roots,
};
use crate::vega::{VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar};
#[path = "predecode_v1/galois_semantic_verifier_v1.rs"]
mod galois_semantic_verifier_v1;
pub(in super::super) use galois_semantic_verifier_v1::{
    CompletedDirectGaloisSemanticVerificationV1, verify_direct_galois_semantic_candidate_v1,
};
#[path = "predecode_v1/rkg_one_semantic_verifier_v1.rs"]
mod rkg_one_semantic_verifier_v1;
pub(in super::super) use rkg_one_semantic_verifier_v1::{
    CompletedDirectRkgOneSemanticVerificationV1, verify_direct_rkg_one_semantic_candidate_v1,
};
pub(super) const MEMBERSHIP_HEADER_BYTES_V1: usize = 339;
pub(super) const BOUND_ONE_CHUNK_WIRE_BYTES_V1: usize = 1_494;
pub(super) const BOUND_TWO_CHUNK_WIRE_BYTES_V1: usize = 1_560;
pub(super) const INNER_COMMITMENT_OFFSET_V1: usize = 12;
const INNER_PROOF_OFFSET_V1: usize = 47;
pub(super) const BORROWED_MEMBERSHIP_PROOF_ALLOCATIONS_ELIDED_V1: usize =
    WITNESS_COUNT_V1 * CHUNKS_PER_WITNESS_V1;
pub(super) const BORROWED_MEMBERSHIP_PROOF_LOGICAL_BYTES_ELIDED_V1: usize =
    2 * CHUNKS_PER_WITNESS_V1 * (BOUND_ONE_CHUNK_WIRE_BYTES_V1 - INNER_PROOF_OFFSET_V1)
        + 4 * CHUNKS_PER_WITNESS_V1 * (BOUND_TWO_CHUNK_WIRE_BYTES_V1 - INNER_PROOF_OFFSET_V1);
const _: () = {
    assert!(BORROWED_MEMBERSHIP_PROOF_ALLOCATIONS_ELIDED_V1 == 48);
    assert!(BORROWED_MEMBERSHIP_PROOF_LOGICAL_BYTES_ELIDED_V1 == 71_568);
};
struct PreflightedDirectRelationMembershipFramesV1<'a> {
    bound_one:
        [PreflightedExactEightChunkMembershipWireV1<'a, DirectRelationBoundOneMembershipRoleV1>; 2],
    bound_two:
        [PreflightedExactEightChunkMembershipWireV1<'a, DirectRelationBoundTwoMembershipRoleV1>; 4],
}
impl<'a> PreflightedDirectRelationMembershipFramesV1<'a> {
    fn preflight(membership: &'a [u8]) -> Result<Self, ZkAmsMkheErrorV1> {
        if membership.len() != MEMBERSHIP_BYTES_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let bound_one = [
            preflight_membership_frame::<DirectRelationBoundOneMembershipRoleV1>(
                membership,
                0,
                DIRECT_BOUND_ONE_MEMBERSHIP_BYTES_V1,
            )?,
            preflight_membership_frame::<DirectRelationBoundOneMembershipRoleV1>(
                membership,
                1,
                DIRECT_BOUND_ONE_MEMBERSHIP_BYTES_V1,
            )?,
        ];
        let bound_two = [
            preflight_membership_frame::<DirectRelationBoundTwoMembershipRoleV1>(
                membership,
                2,
                DIRECT_BOUND_TWO_MEMBERSHIP_BYTES_V1,
            )?,
            preflight_membership_frame::<DirectRelationBoundTwoMembershipRoleV1>(
                membership,
                3,
                DIRECT_BOUND_TWO_MEMBERSHIP_BYTES_V1,
            )?,
            preflight_membership_frame::<DirectRelationBoundTwoMembershipRoleV1>(
                membership,
                4,
                DIRECT_BOUND_TWO_MEMBERSHIP_BYTES_V1,
            )?,
            preflight_membership_frame::<DirectRelationBoundTwoMembershipRoleV1>(
                membership,
                5,
                DIRECT_BOUND_TWO_MEMBERSHIP_BYTES_V1,
            )?,
        ];
        let terminal = MEMBERSHIP_FRAME_OFFSETS_V1[5]
            .checked_add(DIRECT_BOUND_TWO_MEMBERSHIP_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        if terminal != membership.len() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(Self {
            bound_one,
            bound_two,
        })
    }
    fn verify_replayable(&self) -> Result<(), ZkAmsMkheErrorV1> {
        for evidence in &self.bound_one {
            evidence
                .verify_replayable()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        }
        for evidence in &self.bound_two {
            evidence
                .verify_replayable()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        }
        Ok(())
    }
    #[cfg(test)]
    fn verify_replayable_with_for_test<F>(
        &self,
        mut verify_chunk: F,
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        F: FnMut(usize, [u8; 32], u16, &[u8]) -> Result<[u8; 32], ExactEightChunkMembershipErrorV1>,
    {
        for (slot, evidence) in self.bound_one.iter().enumerate() {
            evidence
                .verify_replayable_with_for_test(|context, ordinal, wire| {
                    verify_chunk(slot, context, ordinal, wire)
                })
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        }
        for (index, evidence) in self.bound_two.iter().enumerate() {
            evidence
                .verify_replayable_with_for_test(|context, ordinal, wire| {
                    verify_chunk(index + 2, context, ordinal, wire)
                })
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        }
        Ok(())
    }
    fn copied_commitments(&self) -> [[Point; CHUNKS_PER_WITNESS_V1]; WITNESS_COUNT_V1] {
        [
            *self.bound_one[0].commitments(),
            *self.bound_one[1].commitments(),
            *self.bound_two[0].commitments(),
            *self.bound_two[1].commitments(),
            *self.bound_two[2].commitments(),
            *self.bound_two[3].commitments(),
        ]
    }
    fn validate_expected(
        &self,
        context: ZkAmsMkheDirectCeremonyContextV1,
        capability: &VerifiedPersistentWitnessDirectRelationUseV1,
        expected: &ExpectedDirectRelationStatementV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        for (slot, evidence) in self.bound_one.iter().enumerate() {
            if evidence.context()
                != membership_context::<DirectRelationBoundOneMembershipRoleV1>(
                    context, capability, expected, slot,
                )?
            {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
        }
        for (index, evidence) in self.bound_two.iter().enumerate() {
            let slot = index + 2;
            if evidence.context()
                != membership_context::<DirectRelationBoundTwoMembershipRoleV1>(
                    context, capability, expected, slot,
                )?
            {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
        }
        if self.bound_one[0].commitments() != &capability.secret_commitments {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        match capability.ephemeral_commitments.as_ref() {
            Some(commitments) if self.bound_one[1].commitments() == commitments => {}
            None if expected.relation().ephemeral_consumer().is_none() => {}
            _ => return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
        }
        Ok(())
    }
    #[cfg(test)]
    #[allow(
        clippy::type_complexity,
        reason = "fixed membership arrays preserve reviewed direct-relation tuple order"
    )]
    fn materialize(
        self,
    ) -> Result<
        (
            [ExactEightChunkMembershipEvidenceV1<DirectRelationBoundOneMembershipRoleV1>; 2],
            [ExactEightChunkMembershipEvidenceV1<DirectRelationBoundTwoMembershipRoleV1>; 4],
        ),
        ZkAmsMkheErrorV1,
    > {
        #[cfg(test)]
        OWNED_MEMBERSHIP_MATERIALIZATIONS_V1.with(|count| {
            count.set(count.get().checked_add(1).expect("test counter overflow"));
        });
        let [bound_one_0, bound_one_1] = self.bound_one;
        let [bound_two_0, bound_two_1, bound_two_2, bound_two_3] = self.bound_two;
        Ok((
            [
                bound_one_0
                    .materialize()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                bound_one_1
                    .materialize()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            ],
            [
                bound_two_0
                    .materialize()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                bound_two_1
                    .materialize()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                bound_two_2
                    .materialize()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                bound_two_3
                    .materialize()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            ],
        ))
    }
}
#[cfg(test)]
std::thread_local! {
    static OWNED_MEMBERSHIP_MATERIALIZATIONS_V1: core::cell::Cell<usize> = const {
        core::cell::Cell::new(0)
    };
}
#[cfg(test)]
pub(super) fn reset_owned_membership_materializations_for_test() {
    OWNED_MEMBERSHIP_MATERIALIZATIONS_V1.with(|count| count.set(0));
}
#[cfg(test)]
pub(super) fn owned_membership_materializations_for_test() -> usize {
    OWNED_MEMBERSHIP_MATERIALIZATIONS_V1.with(core::cell::Cell::get)
}
#[cfg(test)]
pub(super) fn preflight_and_materialize_membership_frames_for_test(
    membership: &[u8],
) -> Result<(), ZkAmsMkheErrorV1> {
    let preflighted = PreflightedDirectRelationMembershipFramesV1::preflight(membership)?;
    preflighted.materialize()?;
    Ok(())
}
fn preflight_membership_frame<'a, R: ExactEightChunkMembershipRoleV1>(
    membership: &'a [u8],
    slot: usize,
    frame_bytes: usize,
) -> Result<PreflightedExactEightChunkMembershipWireV1<'a, R>, ZkAmsMkheErrorV1> {
    let start = *MEMBERSHIP_FRAME_OFFSETS_V1
        .get(slot)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let end = start
        .checked_add(frame_bytes)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    PreflightedExactEightChunkMembershipWireV1::preflight(
        membership
            .get(start..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?,
    )
    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)
}
pub(super) fn response_offset(repetition: usize, slot: usize, coefficient: usize) -> Option<usize> {
    (repetition < 4 && slot < 6 && coefficient < 131_072)
        .then_some((repetition * 6 * 131_072 + slot * 131_072 + coefficient) * 8)
}
pub(super) fn blind_response_offset(repetition: usize, slot: usize, chunk: usize) -> Option<usize> {
    (repetition < 4 && slot < 6 && chunk < 8).then_some((repetition * 48 + slot * 8 + chunk) * 32)
}
pub(super) fn membership_commitment_point_offset(slot: usize, chunk: usize) -> Option<usize> {
    if slot >= 6 || chunk >= 8 {
        return None;
    }
    let chunk_wire = if slot < 2 {
        BOUND_ONE_CHUNK_WIRE_BYTES_V1
    } else {
        BOUND_TWO_CHUNK_WIRE_BYTES_V1
    };
    Some(
        MEMBERSHIP_FRAME_OFFSETS_V1[slot]
            + MEMBERSHIP_HEADER_BYTES_V1
            + chunk * chunk_wire
            + INNER_COMMITMENT_OFFSET_V1,
    )
}
#[cfg(test)]
pub(super) fn membership_section_offsets(statement_bytes: usize) -> [usize; 5] {
    let membership_start = HEADER_BYTES_V1 + statement_bytes;
    let response_start = membership_start + MEMBERSHIP_BYTES_V1;
    let blind_start = response_start + RESPONSE_BYTES_V1;
    let seed_start = blind_start + BLIND_RESPONSE_BYTES_V1;
    [
        membership_start,
        response_start,
        blind_start,
        seed_start,
        seed_start + 32,
    ]
}
/// Structural candidate proof retaining one consumed relation capability.
///
/// This type is intentionally neither `Clone` nor publicly inspectable. The
/// large response sections remain borrowed from the caller's immutable bytes.
pub(in super::super) struct UntrustedDirectRelationProofBytesV1<'a> {
    bytes: &'a [u8],
}
impl<'a> UntrustedDirectRelationProofBytesV1<'a> {
    fn new(bytes: &'a [u8]) -> Result<Self, ZkAmsMkheErrorV1> {
        if bytes.len() > ZK_AMS_MKHE_MAX_PROOF_BYTES_V1 {
            return Err(ZkAmsMkheErrorV1::WireTooLarge);
        }
        if bytes.len() < HEADER_BYTES_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(Self { bytes })
    }
}
pub(in super::super) struct PredecodedDirectRelationProofV1<'a> {
    capability: VerifiedPersistentWitnessDirectRelationUseV1,
    relation: PersistentDirectRelationV1,
    statement_digest: [u8; 32],
    transcript_context: ExactBindingTranscriptContextV1,
    membership_frames: PreflightedDirectRelationMembershipFramesV1<'a>,
    responses: &'a [u8],
    blind_responses: &'a [u8],
    challenge_seed: [u8; 32],
}
fn validate_reconstructed_challenge(
    transcript_context: ExactBindingTranscriptContextV1,
    challenge_seed: [u8; 32],
    first_messages: DirectRelationFirstMessageDigestsV1,
) -> Result<[u32; 4], ZkAmsMkheErrorV1> {
    let (seed, challenges) =
        challenge_vector_from_first_messages(transcript_context, first_messages)?;
    if seed != challenge_seed {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(challenges)
}
/// Strictly predecode one exact candidate without proving any relation.
pub(in super::super) fn predecode_direct_relation_proof_v1<'a>(
    context: ZkAmsMkheDirectCeremonyContextV1,
    capability: VerifiedPersistentWitnessDirectRelationUseV1,
    objects: DirectRelationPublicObjectsV1,
    proof_bytes: &'a [u8],
) -> Result<PredecodedDirectRelationProofV1<'a>, ZkAmsMkheErrorV1> {
    capability.validate()?;
    let proof_bytes = UntrustedDirectRelationProofBytesV1::new(proof_bytes)?.bytes;
    let expected = ExpectedDirectRelationStatementV1::new(context, &capability, objects)?;
    validate_header(proof_bytes, &expected)?;
    let statement_start = HEADER_BYTES_V1;
    let statement_end = statement_start
        .checked_add(expected.bytes().len())
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    if proof_bytes.get(statement_start..statement_end) != Some(expected.bytes()) {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let membership_start = statement_end;
    let response_start = membership_start
        .checked_add(MEMBERSHIP_BYTES_V1)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let blind_start = response_start
        .checked_add(RESPONSE_BYTES_V1)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let seed_start = blind_start
        .checked_add(BLIND_RESPONSE_BYTES_V1)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let end = seed_start
        .checked_add(CHALLENGE_SEED_BYTES_V1)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    if end != proof_bytes.len() {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let membership_slice = proof_bytes
        .get(membership_start..response_start)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let responses = proof_bytes
        .get(response_start..blind_start)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let blind_responses = proof_bytes
        .get(blind_start..seed_start)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let challenge_seed = array_at::<32>(proof_bytes, seed_start)?;
    let preflighted = PreflightedDirectRelationMembershipFramesV1::preflight(membership_slice)?;
    preflighted.validate_expected(context, &capability, &expected)?;
    validate_responses(responses)?;
    validate_blind_responses(blind_responses)?;
    if challenge_seed == [0; 32]
        || challenge_seed != capability.selector.proof_commitment_transcript_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let (commitment_set_digest, membership_proof_set_digest) = ordered_membership_roots(
        expected.relation(),
        expected.core_digest(),
        &preflighted.bound_one,
        &preflighted.bound_two,
    );
    let relation_index = (u32::from(capability.selector.evaluated_key_ordinal) << 8)
        | u32::from(capability.selector.digit_index);
    let transcript_context = ExactBindingTranscriptContextV1 {
        profile_digest: context.profile_digest(),
        roster_digest: context.roster_digest(),
        key_material_digest: context.key_material_digest(),
        epoch: context.epoch(),
        protocol_transcript_digest: context.transcript_digest(),
        round_tag: expected.relation() as u8,
        party_index: capability.party_index,
        party: capability.party.to_bytes(),
        record_index: capability.ephemeral_record_index,
        relation_index,
        statement_digest: expected.core_digest(),
        commitment_set_digest,
        membership_proof_set_digest,
        persistent_graph_digest: expected.lineage_digest(),
    };
    transcript_context.validate()?;
    // All six preflighted frames and the candidate proof remain borrowed and
    // live. Avoiding their 48 owned proof buffers elides exactly 71,568
    // logical proof-payload bytes and 48 retained per-proof Vec allocations;
    // this is not a total-allocation, heap, or RSS claim. No move-only
    // authority is constructed by this predecoder.
    Ok(PredecodedDirectRelationProofV1 {
        capability,
        relation: expected.relation(),
        statement_digest: expected.statement_digest(),
        transcript_context,
        membership_frames: preflighted,
        responses,
        blind_responses,
        challenge_seed,
    })
}
pub(super) fn validate_header(
    bytes: &[u8],
    expected: &ExpectedDirectRelationStatementV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let total = HEADER_BYTES_V1
        .checked_add(expected.bytes().len())
        .and_then(|value| value.checked_add(BODY_BYTES_V1))
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let header_is_exact = bytes.get(..HEADER_BYTES_V1)
        == Some(canonical_header(expected).as_slice())
        && bytes.len() == total;
    if header_is_exact {
        Ok(())
    } else {
        Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
    }
}
fn membership_context<R>(
    context: ZkAmsMkheDirectCeremonyContextV1,
    capability: &VerifiedPersistentWitnessDirectRelationUseV1,
    expected: &ExpectedDirectRelationStatementV1,
    slot: usize,
) -> Result<ExactEightChunkMembershipContextV1<R>, ZkAmsMkheErrorV1>
where
    R: super::super::super::exact_eight_chunk_membership::ExactEightChunkMembershipRoleV1,
{
    ExactEightChunkMembershipContextV1::new(
        context.profile_digest(),
        context.roster_digest(),
        context.key_material_digest(),
        context.epoch(),
        context.transcript_digest(),
        capability.party,
        membership_share_statement_digest(expected.relation(), expected.core_digest(), slot),
    )
    .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)
}
fn validate_responses(bytes: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
    if bytes.len() != RESPONSE_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    for response in bytes.chunks_exact(8) {
        validate_response_word(response)?;
    }
    Ok(())
}
fn validate_blind_responses(bytes: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
    if bytes.len() != BLIND_RESPONSE_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    for scalar in bytes.chunks_exact(32) {
        validate_blind_scalar(scalar)?;
    }
    Ok(())
}
pub(super) fn validate_response_word(bytes: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
    let value = i64::from_be_bytes(
        bytes
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
    );
    if value.unsigned_abs() > RESPONSE_COEFFICIENT_BOUND_V1 as u64 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}
pub(super) fn validate_blind_scalar(bytes: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
    Scalar::from_be_bytes_exact(
        bytes
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
    )
    .map(|_| ())
    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)
}
fn array_at<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], ZkAmsMkheErrorV1> {
    bytes
        .get(
            offset
                ..offset
                    .checked_add(N)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?,
        )
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
        .try_into()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)
}
