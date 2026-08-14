//! Candidate-only semantic verification for the direct RKG-round-one relation.
//!
//! This module deliberately returns only an opaque local completion marker.
//! It cannot mint a verification receipt or contribution authentication,
//! admit evaluated-key material, or change any release gate.

#![allow(
    dead_code,
    reason = "candidate-only semantic verifier seam remains disconnected from the fail-closed production verifier"
)]

use super::{PredecodedDirectRelationProofV1, blind_response_offset, response_offset};
use crate::vega::{
    VegaT256PointV1 as Point,
    sponge::keccak256,
    zk_ams::mkhe::{
        ZkAmsMkheErrorV1,
        direct_collective_eval_ceremony::ZkAmsMkheDirectCeremonyContextV1,
        direct_object_transport::{
            ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1, ZkAmsMkheDirectObjectReadAtProviderV1,
        },
        manifest::{
            RELEASE_MODULI_V1, RELEASE_NEGACYCLIC_ROOTS_V1, ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
            release_profile_v1,
        },
        mod_add, mod_mul, mod_pow, mod_sub, negacyclic_multiply, signed_mod,
    },
};

use super::super::super::VerifiedPersistentWitnessDirectRelationUseV1;
use super::super::super::direct_common_a_v1;
use super::super::{
    CHALLENGE_REPETITIONS_V1, CHUNKS_PER_WITNESS_V1, DirectRelationFirstMessageDigestsV1,
    DirectRelationPublicObjectsV1, DirectRelationRnsFirstMessageHasherV1,
    PersistentDirectRelationV1, RECONSTRUCTED_COMMITMENT_BYTES_V1, RELEASE_RING_COEFFICIENTS_V1,
    RELEASE_RNS_LIMBS_V1, WITNESS_COUNT_V1, commitment_first_message_digest,
    response_commitment_v1::reconstruct_direct_response_first_message_v1,
    statement_v1::DirectRkgOneH0H1StatementReplayV1,
};

const RKG_ONE_GADGET_BASE_LOG_V1: u64 = 60;
const RKG_ONE_RETAINED_REPLAY_MATRIX_BYTES_V1: usize =
    2 * RELEASE_RNS_LIMBS_V1 * RELEASE_RING_COEFFICIENTS_V1 * core::mem::size_of::<u64>();
const RKG_ONE_H0_REPLAY_LIMB_BYTES_V1: usize =
    RELEASE_RING_COEFFICIENTS_V1 * core::mem::size_of::<u64>();
const RKG_ONE_RESPONSE_LIMB_BYTES_V1: usize =
    3 * RELEASE_RING_COEFFICIENTS_V1 * core::mem::size_of::<u64>();
const RKG_ONE_ROW_OUTPUT_BYTES_V1: usize =
    RELEASE_RING_COEFFICIENTS_V1 * core::mem::size_of::<u64>();
const RKG_ONE_NEGACYCLIC_PRODUCT_BYTES_V1: usize =
    RELEASE_RING_COEFFICIENTS_V1 * core::mem::size_of::<u64>();
// `negacyclic_multiply` owns exactly two N-limb vectors while its NTT runs;
// one becomes the returned product and the other is released before return.
const RKG_ONE_NEGACYCLIC_NTT_BYTES_V1: usize =
    2 * RELEASE_RING_COEFFICIENTS_V1 * core::mem::size_of::<u64>();
const RKG_ONE_CANONICAL_ENCODING_BYTES_V1: usize =
    RELEASE_RING_COEFFICIENTS_V1 * core::mem::size_of::<u64>();
const RKG_ONE_PAIRED_REPLAY_SCRATCH_BYTES_V1: usize = 8_192;
const RKG_ONE_MEMBERSHIP_PROOF_VERIFICATIONS_V1: usize = WITNESS_COUNT_V1 * CHUNKS_PER_WITNESS_V1;
const RKG_ONE_RESPONSE_COMMITMENT_RECONSTRUCTIONS_V1: usize =
    CHALLENGE_REPETITIONS_V1 * WITNESS_COUNT_V1 * CHUNKS_PER_WITNESS_V1;
const RKG_ONE_NEGACYCLIC_PRODUCTS_V1: usize = 2 * CHALLENGE_REPETITIONS_V1 * RELEASE_RNS_LIMBS_V1;
const RKG_ONE_FORWARD_NTTS_V1: usize = 2 * RKG_ONE_NEGACYCLIC_PRODUCTS_V1;
const RKG_ONE_INVERSE_NTTS_V1: usize = RKG_ONE_NEGACYCLIC_PRODUCTS_V1;
const RKG_ONE_RNS_ROW_COEFFICIENTS_V1: usize =
    4 * CHALLENGE_REPETITIONS_V1 * RELEASE_RNS_LIMBS_V1 * RELEASE_RING_COEFFICIENTS_V1;
const RKG_ONE_PROVIDER_READ_CALLS_V1: usize = 2
    * RELEASE_RNS_LIMBS_V1
    * (RELEASE_RING_COEFFICIENTS_V1
        / (ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 / core::mem::size_of::<u64>()));
const RKG_ONE_PROVIDER_READ_BYTES_V1: usize =
    2 * RELEASE_RNS_LIMBS_V1 * RELEASE_RING_COEFFICIENTS_V1 * core::mem::size_of::<u64>();

// This ledger is deliberately narrow: it accounts only for the exact live
// payload requested by this module's retained replay matrices and RNS
// reconstruction buffers. It excludes the borrowed proof, predecoded owned
// membership evidence, response-MSM transient allocations, allocator
// overhead, replay transaction metadata, and stack frames. It is neither a
// whole-verifier RSS claim nor a release resource certification.
const RKG_ONE_ROW_ZERO_BASE_BYTES_V1: usize = RKG_ONE_RETAINED_REPLAY_MATRIX_BYTES_V1
    + RKG_ONE_H0_REPLAY_LIMB_BYTES_V1
    + RKG_ONE_RESPONSE_LIMB_BYTES_V1;
const RKG_ONE_ROW_ZERO_NTT_LIVE_BYTES_V1: usize =
    RKG_ONE_ROW_ZERO_BASE_BYTES_V1 + RKG_ONE_NEGACYCLIC_NTT_BYTES_V1;
const RKG_ONE_ROW_ZERO_HASH_LIVE_BYTES_V1: usize = RKG_ONE_ROW_ZERO_BASE_BYTES_V1
    + RKG_ONE_NEGACYCLIC_PRODUCT_BYTES_V1
    + RKG_ONE_ROW_OUTPUT_BYTES_V1
    + RKG_ONE_CANONICAL_ENCODING_BYTES_V1;
const RKG_ONE_PAIRED_REPLAY_LIVE_BYTES_V1: usize =
    RKG_ONE_ROW_ZERO_BASE_BYTES_V1 + RKG_ONE_PAIRED_REPLAY_SCRATCH_BYTES_V1;
const RKG_ONE_RNS_LIVE_PAYLOAD_CEILING_BYTES_V1: usize = RKG_ONE_ROW_ZERO_HASH_LIVE_BYTES_V1;
const RKG_ONE_RNS_PAYLOAD_LIMIT_BYTES_V1: usize = 160 * 1024 * 1024;
const _: () = {
    assert!(CHALLENGE_REPETITIONS_V1 == 4);
    assert!(WITNESS_COUNT_V1 == 6);
    assert!(CHUNKS_PER_WITNESS_V1 == 8);
    assert!(RELEASE_RNS_LIMBS_V1 == 38);
    assert!(RELEASE_RING_COEFFICIENTS_V1 == 131_072);
    assert!(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 == RELEASE_RING_COEFFICIENTS_V1);
    assert!(RKG_ONE_RETAINED_REPLAY_MATRIX_BYTES_V1 == 79_691_776);
    assert!(RKG_ONE_H0_REPLAY_LIMB_BYTES_V1 == 1_048_576);
    assert!(RKG_ONE_RESPONSE_LIMB_BYTES_V1 == 3_145_728);
    assert!(RKG_ONE_ROW_OUTPUT_BYTES_V1 == 1_048_576);
    assert!(RKG_ONE_NEGACYCLIC_PRODUCT_BYTES_V1 == 1_048_576);
    assert!(RKG_ONE_NEGACYCLIC_NTT_BYTES_V1 == 2_097_152);
    assert!(RKG_ONE_CANONICAL_ENCODING_BYTES_V1 == 1_048_576);
    assert!(RKG_ONE_ROW_ZERO_BASE_BYTES_V1 == 83_886_080);
    assert!(RKG_ONE_ROW_ZERO_NTT_LIVE_BYTES_V1 == 85_983_232);
    assert!(RKG_ONE_ROW_ZERO_HASH_LIVE_BYTES_V1 == 87_031_808);
    assert!(RKG_ONE_PAIRED_REPLAY_LIVE_BYTES_V1 == 83_894_272);
    assert!(RKG_ONE_RNS_LIVE_PAYLOAD_CEILING_BYTES_V1 == 87_031_808);
    assert!(RKG_ONE_MEMBERSHIP_PROOF_VERIFICATIONS_V1 == 48);
    assert!(RKG_ONE_RESPONSE_COMMITMENT_RECONSTRUCTIONS_V1 == 192);
    assert!(RKG_ONE_NEGACYCLIC_PRODUCTS_V1 == 304);
    assert!(RKG_ONE_FORWARD_NTTS_V1 == 608);
    assert!(RKG_ONE_INVERSE_NTTS_V1 == 304);
    assert!(RKG_ONE_RNS_ROW_COEFFICIENTS_V1 == 79_691_776);
    assert!(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 == 8_192);
    assert!(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1.is_multiple_of(core::mem::size_of::<u64>()));
    assert!(
        RELEASE_RING_COEFFICIENTS_V1
            .is_multiple_of(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 / core::mem::size_of::<u64>())
    );
    assert!(RKG_ONE_PROVIDER_READ_CALLS_V1 == 9_728);
    assert!(RKG_ONE_PROVIDER_READ_BYTES_V1 == 79_691_776);
    assert!(RKG_ONE_RNS_LIVE_PAYLOAD_CEILING_BYTES_V1 < RKG_ONE_RNS_PAYLOAD_LIMIT_BYTES_V1);
};

/// Opaque proof that the candidate RKG-round-one semantic replay completed.
///
/// The marker has no fields, accessors, conversion, or sibling-visible
/// constructor and is deliberately not a release verification receipt.
pub(in super::super::super) struct CompletedDirectRkgOneSemanticVerificationV1 {
    _seal: (),
}

/// Consume one capability and verify an exact RKG-round-one candidate.
pub(in super::super::super) fn verify_direct_rkg_one_semantic_candidate_v1<P>(
    context: ZkAmsMkheDirectCeremonyContextV1,
    capability: VerifiedPersistentWitnessDirectRelationUseV1,
    objects: DirectRelationPublicObjectsV1,
    proof_bytes: &[u8],
    provider: &mut P,
) -> Result<CompletedDirectRkgOneSemanticVerificationV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    let proof =
        super::predecode_direct_relation_proof_v1(context, capability, objects, proof_bytes)?;
    verify_predecoded_direct_rkg_one_semantic_candidate_v1(context, objects, proof, provider)
}

#[allow(
    clippy::drop_non_drop,
    reason = "the explicit post-comparison completion drop is source-contract pinned"
)]
fn verify_predecoded_direct_rkg_one_semantic_candidate_v1<P>(
    context: ZkAmsMkheDirectCeremonyContextV1,
    objects: DirectRelationPublicObjectsV1,
    proof: PredecodedDirectRelationProofV1<'_>,
    provider: &mut P,
) -> Result<CompletedDirectRkgOneSemanticVerificationV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    validate_rns_live_payload_accounting()?;
    if proof.relation != PersistentDirectRelationV1::RkgRoundOne
        || proof.statement_digest == [0; 32]
        || proof.transcript_context.round_tag != PersistentDirectRelationV1::RkgRoundOne as u8
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let DirectRelationPublicObjectsV1::RkgRoundOne { .. } = objects else {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    };
    proof.capability.validate()?;
    let challenges = provisional_challenges(proof.challenge_seed);

    for evidence in &proof.bound_one_membership {
        evidence
            .verify()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    }
    for evidence in &proof.bound_two_membership {
        evidence
            .verify()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    }

    let commitment_digests = reconstruct_commitment_first_messages(
        proof.relation,
        &proof.bound_one_membership,
        &proof.bound_two_membership,
        proof.responses,
        proof.blind_responses,
        challenges,
    )?;

    let mut retained_replay_matrices = try_zeroed_u64(retained_replay_matrix_words()?)?;
    let mut rns_hashers: [DirectRelationRnsFirstMessageHasherV1; CHALLENGE_REPETITIONS_V1] =
        core::array::from_fn(|_| {
            DirectRelationRnsFirstMessageHasherV1::new(PersistentDirectRelationV1::RkgRoundOne)
        });
    let mut common_a =
        direct_common_a_v1::DirectCommonAReplayV1::begin(context, &proof.capability)?;
    let mut public =
        DirectRkgOneH0H1StatementReplayV1::begin(context, &proof.capability, objects, provider)?;
    replay_rkg_one_retained_matrices(
        context,
        provider,
        &mut common_a,
        &mut public,
        &mut retained_replay_matrices,
        proof.responses,
        challenges,
        &mut rns_hashers,
    )?;

    // Both opaque completions remain live through the final transcript
    // comparison. Neither authority object or snapshot identity escapes.
    let completed_replays = (common_a.finish()?, public.finish(provider)?);
    let rns_digests = reconstruct_rkg_one_rns_first_messages(
        proof.responses,
        challenges,
        &retained_replay_matrices,
        &mut rns_hashers,
    )?;
    let first_messages = DirectRelationFirstMessageDigestsV1::new(rns_digests, commitment_digests)?;
    let reconstructed_challenges = proof.validate_reconstructed_challenge(first_messages)?;
    if reconstructed_challenges != challenges {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    drop(completed_replays);
    Ok(CompletedDirectRkgOneSemanticVerificationV1 { _seal: () })
}

fn provisional_challenges(challenge_seed: [u8; 32]) -> [u32; CHALLENGE_REPETITIONS_V1] {
    core::array::from_fn(|ordinal| {
        let mut frame =
            Vec::with_capacity(super::super::super::CHALLENGE_COORDINATE_DOMAIN_V1.len() + 33);
        frame.extend_from_slice(super::super::super::CHALLENGE_COORDINATE_DOMAIN_V1);
        frame.extend_from_slice(&challenge_seed);
        frame.push(ordinal as u8);
        u32::from_be_bytes(
            keccak256(&frame)[..4]
                .try_into()
                .expect("four-byte challenge prefix"),
        )
    })
}

#[allow(
    clippy::needless_range_loop,
    reason = "fixed protocol chunk indices are source-contract pinned"
)]
fn reconstruct_commitment_first_messages<BoundOne, BoundTwo>(
    relation: PersistentDirectRelationV1,
    bound_one: &[BoundOne; 2],
    bound_two: &[BoundTwo; 4],
    responses: &[u8],
    blind_responses: &[u8],
    challenges: [u32; CHALLENGE_REPETITIONS_V1],
) -> Result<[[u8; 32]; CHALLENGE_REPETITIONS_V1], ZkAmsMkheErrorV1>
where
    BoundOne: MembershipCommitmentsV1,
    BoundTwo: MembershipCommitmentsV1,
{
    let mut digests = [[0_u8; 32]; CHALLENGE_REPETITIONS_V1];
    for repetition in 0..CHALLENGE_REPETITIONS_V1 {
        let mut encoded = [0_u8; RECONSTRUCTED_COMMITMENT_BYTES_V1];
        let mut cursor: usize = 0;
        for slot in 0..WITNESS_COUNT_V1 {
            let commitments = if slot < 2 {
                bound_one[slot].commitments()
            } else {
                bound_two[slot - 2].commitments()
            };
            for chunk in 0..CHUNKS_PER_WITNESS_V1 {
                let coefficient_start = chunk * super::super::super::WITNESS_CHUNK_COEFFICIENTS_V1;
                let response_start = response_offset(repetition, slot, coefficient_start)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
                let response_chunk_bytes = super::super::super::WITNESS_CHUNK_COEFFICIENTS_V1
                    .checked_mul(core::mem::size_of::<i64>())
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
                let response_end = response_start
                    .checked_add(response_chunk_bytes)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
                let response_chunk = responses
                    .get(response_start..response_end)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
                let coefficients = decode_response_chunk(response_chunk)?;
                let blind_start = blind_response_offset(repetition, slot, chunk)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
                let blind_end = blind_start
                    .checked_add(32)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
                let blind: &[u8; 32] = blind_responses
                    .get(blind_start..blind_end)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
                    .try_into()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
                let point = reconstruct_direct_response_first_message_v1(
                    &coefficients,
                    blind,
                    challenges[repetition],
                    &commitments[chunk],
                )
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
                let point_end = cursor
                    .checked_add(point.as_bytes().len())
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
                encoded
                    .get_mut(cursor..point_end)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
                    .copy_from_slice(point.as_bytes());
                cursor = point_end;
            }
        }
        if cursor != encoded.len() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        digests[repetition] = commitment_first_message_digest(relation, &encoded)?;
    }
    Ok(digests)
}

trait MembershipCommitmentsV1 {
    fn commitments(&self) -> [Point; CHUNKS_PER_WITNESS_V1];
}

impl<R> MembershipCommitmentsV1
    for crate::vega::zk_ams::mkhe::exact_eight_chunk_membership::ExactEightChunkMembershipEvidenceV1<
        R,
    >
where
    R: crate::vega::zk_ams::mkhe::exact_eight_chunk_membership::ExactEightChunkMembershipRoleV1,
{
    fn commitments(&self) -> [Point; CHUNKS_PER_WITNESS_V1] {
        crate::vega::zk_ams::mkhe::exact_eight_chunk_membership::ExactEightChunkMembershipEvidenceV1::<
            R,
        >::commitments(self)
    }
}

fn decode_response_chunk(
    bytes: &[u8],
) -> Result<[i64; super::super::super::WITNESS_CHUNK_COEFFICIENTS_V1], ZkAmsMkheErrorV1> {
    let expected = super::super::super::WITNESS_CHUNK_COEFFICIENTS_V1
        .checked_mul(core::mem::size_of::<i64>())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if bytes.len() != expected {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let mut output = [0_i64; super::super::super::WITNESS_CHUNK_COEFFICIENTS_V1];
    for (coefficient, encoded) in output.iter_mut().zip(bytes.chunks_exact(8)) {
        *coefficient = i64::from_be_bytes(
            encoded
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
        );
    }
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
fn replay_rkg_one_retained_matrices<P>(
    context: ZkAmsMkheDirectCeremonyContextV1,
    provider: &mut P,
    common_a: &mut direct_common_a_v1::DirectCommonAReplayV1,
    public: &mut DirectRkgOneH0H1StatementReplayV1,
    retained_replay_matrices: &mut [u64],
    responses: &[u8],
    challenges: [u32; CHALLENGE_REPETITIONS_V1],
    hashers: &mut [DirectRelationRnsFirstMessageHasherV1; CHALLENGE_REPETITIONS_V1],
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    let retained_words = retained_replay_matrix_words()?;
    if retained_replay_matrices.len() != retained_words {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let digit = usize::from(context.digit_index());
    if digit >= RELEASE_RNS_LIMBS_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let (a_matrix, h1_matrix) = retained_replay_matrices.split_at_mut(replay_matrix_words()?);
    let mut h0 = try_zeroed_u64(RELEASE_RING_COEFFICIENTS_V1)?;
    let mut response = try_zeroed_u64(response_limb_words(3)?)?;
    let release_profile = release_profile_v1();
    for (limb, (&modulus, root)) in RELEASE_MODULI_V1
        .iter()
        .zip(RELEASE_NEGACYCLIC_ROOTS_V1)
        .enumerate()
    {
        let a = matrix_limb_mut(a_matrix, limb)?;
        let h1 = matrix_limb_mut(h1_matrix, limb)?;
        common_a.derive_next_limb_into(a)?;
        public.replay_next_limb_pair_into(provider, &mut h0, h1)?;
        let gadget = gadget_residue(digit, modulus)?;
        let plaintext_multiplier = release_profile.plaintext_modulus.residue(modulus);
        for repetition in 0..CHALLENGE_REPETITIONS_V1 {
            decode_response_slots(responses, repetition, &[0, 1, 2], modulus, &mut response)?;
            let (s, tail) = response.split_at(RELEASE_RING_COEFFICIENTS_V1);
            let (u, e0) = tail.split_at(RELEASE_RING_COEFFICIENTS_V1);
            let a_times_u = negacyclic_multiply(a, u, modulus, root)?;
            let mut row = try_zeroed_u64(RELEASE_RING_COEFFICIENTS_V1)?;
            let challenge = u64::from(challenges[repetition]) % modulus;
            for coefficient in 0..RELEASE_RING_COEFFICIENTS_V1 {
                row[coefficient] = reconstruct_h0_coefficient(
                    a_times_u[coefficient],
                    mod_mul(s[coefficient], gadget, modulus),
                    mod_mul(e0[coefficient], plaintext_multiplier, modulus),
                    mod_mul(challenge, h0[coefficient], modulus),
                    modulus,
                );
            }
            absorb_residue_limb(&mut hashers[repetition], 0, limb, &row)?;
        }
    }
    Ok(())
}

#[allow(
    clippy::needless_range_loop,
    reason = "fixed challenge repetition indices are source-contract pinned"
)]
fn reconstruct_rkg_one_rns_first_messages(
    responses: &[u8],
    challenges: [u32; CHALLENGE_REPETITIONS_V1],
    retained_replay_matrices: &[u64],
    hashers: &mut [DirectRelationRnsFirstMessageHasherV1; CHALLENGE_REPETITIONS_V1],
) -> Result<[[u8; 32]; CHALLENGE_REPETITIONS_V1], ZkAmsMkheErrorV1> {
    let retained_words = retained_replay_matrix_words()?;
    if retained_replay_matrices.len() != retained_words {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let (a_matrix, h1_matrix) = retained_replay_matrices.split_at(replay_matrix_words()?);
    let mut response = try_zeroed_u64(response_limb_words(3)?)?;
    let release_profile = release_profile_v1();
    for (limb, (&modulus, root)) in RELEASE_MODULI_V1
        .iter()
        .zip(RELEASE_NEGACYCLIC_ROOTS_V1)
        .enumerate()
    {
        let a = matrix_limb(a_matrix, limb)?;
        let h1 = matrix_limb(h1_matrix, limb)?;
        let plaintext_multiplier = release_profile.plaintext_modulus.residue(modulus);
        for repetition in 0..CHALLENGE_REPETITIONS_V1 {
            decode_response_slots(
                responses,
                repetition,
                &[0, 3],
                modulus,
                &mut response[..2 * RELEASE_RING_COEFFICIENTS_V1],
            )?;
            let (s, e1) =
                response[..2 * RELEASE_RING_COEFFICIENTS_V1].split_at(RELEASE_RING_COEFFICIENTS_V1);
            let a_times_s = negacyclic_multiply(a, s, modulus, root)?;
            let mut row = try_zeroed_u64(RELEASE_RING_COEFFICIENTS_V1)?;
            let challenge = u64::from(challenges[repetition]) % modulus;
            for coefficient in 0..RELEASE_RING_COEFFICIENTS_V1 {
                row[coefficient] = reconstruct_h1_coefficient(
                    a_times_s[coefficient],
                    mod_mul(e1[coefficient], plaintext_multiplier, modulus),
                    mod_mul(challenge, h1[coefficient], modulus),
                    modulus,
                );
            }
            absorb_residue_limb(&mut hashers[repetition], 1, limb, &row)?;
        }
    }
    // Tags 0x84/0x85 are equations for forced-zero witness slots. Their
    // first messages are z4-c*0=z4 and z5-c*0=z5, never an all-zero row.
    for (row, slot) in [(2, 4), (3, 5)] {
        for (limb, modulus) in RELEASE_MODULI_V1.iter().copied().enumerate() {
            for repetition in 0..CHALLENGE_REPETITIONS_V1 {
                decode_response_slots(
                    responses,
                    repetition,
                    &[slot],
                    modulus,
                    &mut response[..RELEASE_RING_COEFFICIENTS_V1],
                )?;
                absorb_residue_limb(
                    &mut hashers[repetition],
                    row,
                    limb,
                    &response[..RELEASE_RING_COEFFICIENTS_V1],
                )?;
            }
        }
    }
    let hashers = core::mem::replace(
        hashers,
        core::array::from_fn(|_| {
            DirectRelationRnsFirstMessageHasherV1::new(PersistentDirectRelationV1::RkgRoundOne)
        }),
    );
    let mut digests = [[0_u8; 32]; CHALLENGE_REPETITIONS_V1];
    for (digest, hasher) in digests.iter_mut().zip(hashers) {
        *digest = hasher.finish()?;
    }
    Ok(digests)
}

fn decode_response_slots(
    responses: &[u8],
    repetition: usize,
    slots: &[usize],
    modulus: u64,
    destination: &mut [u64],
) -> Result<(), ZkAmsMkheErrorV1> {
    if repetition >= CHALLENGE_REPETITIONS_V1
        || slots.is_empty()
        || slots.len() > 3
        || slots.iter().any(|slot| *slot >= WITNESS_COUNT_V1)
        || destination.len()
            != slots
                .len()
                .checked_mul(RELEASE_RING_COEFFICIENTS_V1)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    for (destination_slot, slot) in destination
        .chunks_exact_mut(RELEASE_RING_COEFFICIENTS_V1)
        .zip(slots)
    {
        for (coefficient, residue) in destination_slot.iter_mut().enumerate() {
            let source = response_offset(repetition, *slot, coefficient)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            let end = source
                .checked_add(core::mem::size_of::<i64>())
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            let encoded = responses
                .get(source..end)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            let value = i64::from_be_bytes(
                encoded
                    .try_into()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            );
            *residue = signed_mod(value, modulus);
        }
    }
    Ok(())
}

fn absorb_residue_limb(
    hasher: &mut DirectRelationRnsFirstMessageHasherV1,
    row: usize,
    limb: usize,
    residues: &[u64],
) -> Result<(), ZkAmsMkheErrorV1> {
    if residues.len() != RELEASE_RING_COEFFICIENTS_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let encoding_bytes = RELEASE_RING_COEFFICIENTS_V1
        .checked_mul(core::mem::size_of::<u64>())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut bytes = try_zeroed_bytes(encoding_bytes)?;
    for (encoded, residue) in bytes.chunks_exact_mut(8).zip(residues) {
        encoded.copy_from_slice(&residue.to_be_bytes());
    }
    hasher.absorb_limb(row, limb, &bytes)
}

fn matrix_limb(matrix: &[u64], limb: usize) -> Result<&[u64], ZkAmsMkheErrorV1> {
    let start = limb
        .checked_mul(RELEASE_RING_COEFFICIENTS_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let end = start
        .checked_add(RELEASE_RING_COEFFICIENTS_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    matrix
        .get(start..end)
        .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)
}

fn matrix_limb_mut(matrix: &mut [u64], limb: usize) -> Result<&mut [u64], ZkAmsMkheErrorV1> {
    let start = limb
        .checked_mul(RELEASE_RING_COEFFICIENTS_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let end = start
        .checked_add(RELEASE_RING_COEFFICIENTS_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    matrix
        .get_mut(start..end)
        .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)
}

fn replay_matrix_words() -> Result<usize, ZkAmsMkheErrorV1> {
    RELEASE_RNS_LIMBS_V1
        .checked_mul(RELEASE_RING_COEFFICIENTS_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn retained_replay_matrix_words() -> Result<usize, ZkAmsMkheErrorV1> {
    replay_matrix_words()?
        .checked_mul(2)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn response_limb_words(limbs: usize) -> Result<usize, ZkAmsMkheErrorV1> {
    limbs
        .checked_mul(RELEASE_RING_COEFFICIENTS_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn reconstruct_h0_coefficient(
    a_times_u: u64,
    gadget_times_s: u64,
    plaintext_times_e0: u64,
    challenge_times_h0: u64,
    modulus: u64,
) -> u64 {
    mod_sub(
        mod_add(
            mod_sub(gadget_times_s, a_times_u, modulus),
            plaintext_times_e0,
            modulus,
        ),
        challenge_times_h0,
        modulus,
    )
}

fn reconstruct_h1_coefficient(
    a_times_s: u64,
    plaintext_times_e1: u64,
    challenge_times_h1: u64,
    modulus: u64,
) -> u64 {
    mod_sub(
        mod_add(a_times_s, plaintext_times_e1, modulus),
        challenge_times_h1,
        modulus,
    )
}

fn gadget_residue(digit: usize, modulus: u64) -> Result<u64, ZkAmsMkheErrorV1> {
    if digit >= RELEASE_RNS_LIMBS_V1 || modulus < 3 {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(mod_pow(
        mod_pow(2, RKG_ONE_GADGET_BASE_LOG_V1, modulus),
        u64::try_from(digit).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        modulus,
    ))
}

fn validate_rns_live_payload_accounting() -> Result<(), ZkAmsMkheErrorV1> {
    let retained = retained_replay_matrix_words()?
        .checked_mul(core::mem::size_of::<u64>())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let row_zero_base = retained
        .checked_add(RKG_ONE_H0_REPLAY_LIMB_BYTES_V1)
        .and_then(|value| value.checked_add(RKG_ONE_RESPONSE_LIMB_BYTES_V1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let row_zero_ntt = row_zero_base
        .checked_add(RKG_ONE_NEGACYCLIC_NTT_BYTES_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let row_zero_hash = row_zero_base
        .checked_add(RKG_ONE_NEGACYCLIC_PRODUCT_BYTES_V1)
        .and_then(|value| value.checked_add(RKG_ONE_ROW_OUTPUT_BYTES_V1))
        .and_then(|value| value.checked_add(RKG_ONE_CANONICAL_ENCODING_BYTES_V1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let paired_replay = row_zero_base
        .checked_add(RKG_ONE_PAIRED_REPLAY_SCRATCH_BYTES_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let peak = row_zero_ntt.max(row_zero_hash).max(paired_replay);
    if retained != RKG_ONE_RETAINED_REPLAY_MATRIX_BYTES_V1
        || row_zero_base != RKG_ONE_ROW_ZERO_BASE_BYTES_V1
        || row_zero_ntt != RKG_ONE_ROW_ZERO_NTT_LIVE_BYTES_V1
        || row_zero_hash != RKG_ONE_ROW_ZERO_HASH_LIVE_BYTES_V1
        || paired_replay != RKG_ONE_PAIRED_REPLAY_LIVE_BYTES_V1
        || peak != RKG_ONE_RNS_LIVE_PAYLOAD_CEILING_BYTES_V1
        || peak >= RKG_ONE_RNS_PAYLOAD_LIMIT_BYTES_V1
    {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    Ok(())
}

fn try_zeroed_u64(length: usize) -> Result<Vec<u64>, ZkAmsMkheErrorV1> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    output.resize(length, 0);
    Ok(output)
}

fn try_zeroed_bytes(length: usize) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    output.resize(length, 0);
    Ok(output)
}

#[cfg(test)]
#[path = "rkg_one_semantic_verifier_v1_tests.rs"]
mod tests;
