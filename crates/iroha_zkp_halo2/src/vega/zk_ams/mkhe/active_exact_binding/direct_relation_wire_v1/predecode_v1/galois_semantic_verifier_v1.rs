//! Candidate-only semantic verification for the direct Galois relation.
//!
//! This module returns only an opaque local completion marker. It cannot mint
//! a verification receipt, admit evaluated-key material, or change any gate.

#![allow(
    dead_code,
    reason = "candidate-only Galois verifier remains disconnected from the fail-closed production verifier"
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

use super::super::super::{
    VerifiedPersistentWitnessDirectRelationUseV1, direct_galois_target_a_v1,
};
use super::super::{
    CHALLENGE_REPETITIONS_V1, CHUNKS_PER_WITNESS_V1, DirectRelationFirstMessageDigestsV1,
    DirectRelationPublicObjectsV1, DirectRelationRnsFirstMessageHasherV1,
    PersistentDirectRelationV1, RECONSTRUCTED_COMMITMENT_BYTES_V1, RELEASE_RING_COEFFICIENTS_V1,
    RELEASE_RNS_LIMBS_V1, WITNESS_COUNT_V1, commitment_first_message_digest,
    response_commitment_v1::reconstruct_direct_response_first_message_v1,
    statement_v1::DirectGaloisBStatementReplayV1,
};

const GALOIS_ACTIVE_RESPONSE_SLOTS_V1: [usize; 2] = [0, 2];
const GALOIS_FORCED_ROW_SLOTS_V1: [(usize, usize); 4] = [(1, 1), (2, 3), (3, 4), (4, 5)];
const GALOIS_TARGET_A_LIMB_BYTES_V1: usize =
    RELEASE_RING_COEFFICIENTS_V1 * core::mem::size_of::<u64>();
const GALOIS_B_REPLAY_LIMB_BYTES_V1: usize = GALOIS_TARGET_A_LIMB_BYTES_V1;
const GALOIS_RESPONSE_LIMB_BYTES_V1: usize =
    2 * RELEASE_RING_COEFFICIENTS_V1 * core::mem::size_of::<u64>();
const GALOIS_ROW_OUTPUT_BYTES_V1: usize = GALOIS_TARGET_A_LIMB_BYTES_V1;
const GALOIS_NEGACYCLIC_PRODUCT_BYTES_V1: usize = GALOIS_TARGET_A_LIMB_BYTES_V1;
// `negacyclic_multiply` owns exactly two N-limb vectors while its NTT runs.
const GALOIS_NEGACYCLIC_NTT_BYTES_V1: usize = 2 * GALOIS_TARGET_A_LIMB_BYTES_V1;
const GALOIS_CANONICAL_ENCODING_BYTES_V1: usize = GALOIS_TARGET_A_LIMB_BYTES_V1;
const GALOIS_B_REPLAY_SCRATCH_BYTES_V1: usize = ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1;
const GALOIS_MEMBERSHIP_PROOF_VERIFICATIONS_V1: usize = WITNESS_COUNT_V1 * CHUNKS_PER_WITNESS_V1;
const GALOIS_RESPONSE_COMMITMENT_RECONSTRUCTIONS_V1: usize =
    CHALLENGE_REPETITIONS_V1 * WITNESS_COUNT_V1 * CHUNKS_PER_WITNESS_V1;
const GALOIS_NEGACYCLIC_PRODUCTS_V1: usize = CHALLENGE_REPETITIONS_V1 * RELEASE_RNS_LIMBS_V1;
const GALOIS_FORWARD_NTTS_V1: usize = 2 * GALOIS_NEGACYCLIC_PRODUCTS_V1;
const GALOIS_INVERSE_NTTS_V1: usize = GALOIS_NEGACYCLIC_PRODUCTS_V1;
const GALOIS_AUTOMORPHISM_SCATTERS_V1: usize = GALOIS_NEGACYCLIC_PRODUCTS_V1;
const GALOIS_RNS_ROW_COEFFICIENTS_V1: usize =
    5 * CHALLENGE_REPETITIONS_V1 * RELEASE_RNS_LIMBS_V1 * RELEASE_RING_COEFFICIENTS_V1;
const GALOIS_TARGET_A_DERIVED_BYTES_V1: usize =
    RELEASE_RNS_LIMBS_V1 * RELEASE_RING_COEFFICIENTS_V1 * core::mem::size_of::<u64>();
const GALOIS_PROVIDER_READ_CALLS_V1: usize = RELEASE_RNS_LIMBS_V1
    * (RELEASE_RING_COEFFICIENTS_V1
        / (ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 / core::mem::size_of::<u64>()));
const GALOIS_PROVIDER_READ_BYTES_V1: usize = GALOIS_TARGET_A_DERIVED_BYTES_V1;

// This is a deliberately narrow live-payload ledger. It covers only this
// module's limb, product, row, encoding, and typed-B replay buffers. It omits
// borrowed proof bytes, owned membership evidence, response-MSM transients,
// target-A sampler frames, allocator overhead, transaction metadata, and stack
// frames. It is not a whole-verifier RSS claim or release certification.
const GALOIS_ROW_ZERO_BASE_BYTES_V1: usize =
    GALOIS_TARGET_A_LIMB_BYTES_V1 + GALOIS_B_REPLAY_LIMB_BYTES_V1 + GALOIS_RESPONSE_LIMB_BYTES_V1;
const GALOIS_ROW_ZERO_NTT_LIVE_BYTES_V1: usize =
    GALOIS_ROW_ZERO_BASE_BYTES_V1 + GALOIS_NEGACYCLIC_NTT_BYTES_V1;
const GALOIS_ROW_ZERO_ASSEMBLY_LIVE_BYTES_V1: usize =
    GALOIS_ROW_ZERO_BASE_BYTES_V1 + GALOIS_NEGACYCLIC_PRODUCT_BYTES_V1 + GALOIS_ROW_OUTPUT_BYTES_V1;
const GALOIS_ROW_ZERO_HASH_LIVE_BYTES_V1: usize =
    GALOIS_ROW_ZERO_BASE_BYTES_V1 + GALOIS_ROW_OUTPUT_BYTES_V1 + GALOIS_CANONICAL_ENCODING_BYTES_V1;
const GALOIS_TYPED_B_REPLAY_LIVE_BYTES_V1: usize =
    GALOIS_ROW_ZERO_BASE_BYTES_V1 + GALOIS_B_REPLAY_SCRATCH_BYTES_V1;
const GALOIS_FORCED_ROW_LIVE_BYTES_V1: usize =
    GALOIS_TARGET_A_LIMB_BYTES_V1 + GALOIS_CANONICAL_ENCODING_BYTES_V1;
const GALOIS_RNS_LIVE_PAYLOAD_CEILING_BYTES_V1: usize = GALOIS_ROW_ZERO_NTT_LIVE_BYTES_V1;
const GALOIS_RNS_PAYLOAD_LIMIT_BYTES_V1: usize = 16 * 1024 * 1024;
const _: () = {
    assert!(CHALLENGE_REPETITIONS_V1 == 4);
    assert!(WITNESS_COUNT_V1 == 6);
    assert!(CHUNKS_PER_WITNESS_V1 == 8);
    assert!(RELEASE_RNS_LIMBS_V1 == 38);
    assert!(RELEASE_RING_COEFFICIENTS_V1 == 131_072);
    assert!(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 == RELEASE_RING_COEFFICIENTS_V1);
    assert!(GALOIS_TARGET_A_LIMB_BYTES_V1 == 1_048_576);
    assert!(GALOIS_B_REPLAY_LIMB_BYTES_V1 == 1_048_576);
    assert!(GALOIS_RESPONSE_LIMB_BYTES_V1 == 2_097_152);
    assert!(GALOIS_ROW_ZERO_BASE_BYTES_V1 == 4_194_304);
    assert!(GALOIS_ROW_ZERO_NTT_LIVE_BYTES_V1 == 6_291_456);
    assert!(GALOIS_ROW_ZERO_ASSEMBLY_LIVE_BYTES_V1 == 6_291_456);
    assert!(GALOIS_ROW_ZERO_HASH_LIVE_BYTES_V1 == 6_291_456);
    assert!(GALOIS_TYPED_B_REPLAY_LIVE_BYTES_V1 == 4_202_496);
    assert!(GALOIS_FORCED_ROW_LIVE_BYTES_V1 == 2_097_152);
    assert!(GALOIS_RNS_LIVE_PAYLOAD_CEILING_BYTES_V1 == 6_291_456);
    assert!(GALOIS_MEMBERSHIP_PROOF_VERIFICATIONS_V1 == 48);
    assert!(GALOIS_RESPONSE_COMMITMENT_RECONSTRUCTIONS_V1 == 192);
    assert!(GALOIS_NEGACYCLIC_PRODUCTS_V1 == 152);
    assert!(GALOIS_FORWARD_NTTS_V1 == 304);
    assert!(GALOIS_INVERSE_NTTS_V1 == 152);
    assert!(GALOIS_AUTOMORPHISM_SCATTERS_V1 == 152);
    assert!(GALOIS_RNS_ROW_COEFFICIENTS_V1 == 99_614_720);
    assert!(GALOIS_TARGET_A_DERIVED_BYTES_V1 == 39_845_888);
    assert!(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 == 8_192);
    assert!(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1.is_multiple_of(core::mem::size_of::<u64>()));
    assert!(
        RELEASE_RING_COEFFICIENTS_V1
            .is_multiple_of(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 / core::mem::size_of::<u64>())
    );
    assert!(GALOIS_PROVIDER_READ_CALLS_V1 == 4_864);
    assert!(GALOIS_PROVIDER_READ_BYTES_V1 == 39_845_888);
    assert!(GALOIS_RNS_LIVE_PAYLOAD_CEILING_BYTES_V1 < GALOIS_RNS_PAYLOAD_LIMIT_BYTES_V1);
};

/// Opaque proof that candidate Galois semantic replay completed.
pub(in super::super::super) struct CompletedDirectGaloisSemanticVerificationV1 {
    _seal: (),
}

/// Consume one capability and verify an exact Galois candidate.
pub(in super::super::super) fn verify_direct_galois_semantic_candidate_v1<P>(
    context: ZkAmsMkheDirectCeremonyContextV1,
    capability: VerifiedPersistentWitnessDirectRelationUseV1,
    objects: DirectRelationPublicObjectsV1,
    proof_bytes: &[u8],
    provider: &mut P,
) -> Result<CompletedDirectGaloisSemanticVerificationV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    let proof =
        super::predecode_direct_relation_proof_v1(context, capability, objects, proof_bytes)?;
    verify_predecoded_direct_galois_semantic_candidate_v1(context, objects, proof, provider)
}

fn verify_predecoded_direct_galois_semantic_candidate_v1<P>(
    context: ZkAmsMkheDirectCeremonyContextV1,
    objects: DirectRelationPublicObjectsV1,
    proof: PredecodedDirectRelationProofV1<'_>,
    provider: &mut P,
) -> Result<CompletedDirectGaloisSemanticVerificationV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    validate_rns_live_payload_accounting()?;
    if proof.relation != PersistentDirectRelationV1::Galois
        || proof.statement_digest == [0; 32]
        || proof.transcript_context.round_tag != PersistentDirectRelationV1::Galois as u8
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let DirectRelationPublicObjectsV1::Galois { .. } = objects else {
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

    let mut hashers: [DirectRelationRnsFirstMessageHasherV1; CHALLENGE_REPETITIONS_V1] =
        core::array::from_fn(|_| {
            DirectRelationRnsFirstMessageHasherV1::new(PersistentDirectRelationV1::Galois)
        });
    let mut target_a =
        direct_galois_target_a_v1::DirectGaloisTargetAReplayV1::begin(context, &proof.capability)?;
    let mut public_b =
        DirectGaloisBStatementReplayV1::begin(context, &proof.capability, objects, provider)?;
    replay_galois_row_zero(
        context,
        provider,
        &mut target_a,
        &mut public_b,
        proof.responses,
        challenges,
        &mut hashers,
    )?;

    // Both opaque completions stay live through the final challenge equality.
    let completed_replays = (target_a.finish()?, public_b.finish(provider)?);
    reconstruct_forced_zero_rows(proof.responses, &mut hashers)?;
    let rns_digests = finish_rns_hashers(&mut hashers)?;
    let first_messages = DirectRelationFirstMessageDigestsV1::new(rns_digests, commitment_digests)?;
    let reconstructed_challenges = proof.validate_reconstructed_challenge(first_messages)?;
    if reconstructed_challenges != challenges {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    drop(completed_replays);
    Ok(CompletedDirectGaloisSemanticVerificationV1 { _seal: () })
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
                let response_bytes = super::super::super::WITNESS_CHUNK_COEFFICIENTS_V1
                    .checked_mul(core::mem::size_of::<i64>())
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
                let response_end = response_start
                    .checked_add(response_bytes)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
                let coefficients = decode_response_chunk(
                    responses
                        .get(response_start..response_end)
                        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                )?;
                let blind_start = blind_response_offset(repetition, slot, chunk)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
                let blind: &[u8; 32] = blind_responses
                    .get(
                        blind_start
                            ..blind_start
                                .checked_add(32)
                                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                    )
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
fn replay_galois_row_zero<P>(
    context: ZkAmsMkheDirectCeremonyContextV1,
    provider: &mut P,
    target_a: &mut direct_galois_target_a_v1::DirectGaloisTargetAReplayV1,
    public_b: &mut DirectGaloisBStatementReplayV1,
    responses: &[u8],
    challenges: [u32; CHALLENGE_REPETITIONS_V1],
    hashers: &mut [DirectRelationRnsFirstMessageHasherV1; CHALLENGE_REPETITIONS_V1],
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    let profile = release_profile_v1();
    profile.validate()?;
    let digit = usize::from(context.digit_index());
    let exponent = usize::try_from(context.galois_exponent())
        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    let twice_degree = RELEASE_RING_COEFFICIENTS_V1
        .checked_mul(2)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if digit >= profile.gadget_digits
        || exponent <= 1
        || exponent >= twice_degree
        || exponent.is_multiple_of(2)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut a = try_zeroed_u64(RELEASE_RING_COEFFICIENTS_V1)?;
    let mut b = try_zeroed_u64(RELEASE_RING_COEFFICIENTS_V1)?;
    let mut response = try_zeroed_u64(2 * RELEASE_RING_COEFFICIENTS_V1)?;
    for (limb, (&modulus, root)) in RELEASE_MODULI_V1
        .iter()
        .zip(RELEASE_NEGACYCLIC_ROOTS_V1)
        .enumerate()
    {
        target_a.derive_next_limb_into(&mut a)?;
        public_b.replay_next_limb_into(provider, &mut b)?;
        let gadget = gadget_residue(digit, profile.gadget_base_log, modulus)?;
        let plaintext_multiplier = profile.plaintext_modulus.residue(modulus);
        for repetition in 0..CHALLENGE_REPETITIONS_V1 {
            decode_response_slots(
                responses,
                repetition,
                &GALOIS_ACTIVE_RESPONSE_SLOTS_V1,
                modulus,
                &mut response,
            )?;
            let (s, e) = response.split_at(RELEASE_RING_COEFFICIENTS_V1);
            let a_times_s = negacyclic_multiply(&a, s, modulus, root)?;
            let mut row = try_zeroed_u64(RELEASE_RING_COEFFICIENTS_V1)?;
            scatter_gadget_automorphism(s, exponent, gadget, modulus, &mut row)?;
            let challenge = u64::from(challenges[repetition]) % modulus;
            for coefficient in 0..RELEASE_RING_COEFFICIENTS_V1 {
                row[coefficient] = reconstruct_galois_b_coefficient(
                    a_times_s[coefficient],
                    row[coefficient],
                    mod_mul(e[coefficient], plaintext_multiplier, modulus),
                    mod_mul(challenge, b[coefficient], modulus),
                    modulus,
                );
            }
            drop(a_times_s);
            absorb_residue_limb(&mut hashers[repetition], 0, limb, &row)?;
        }
    }
    Ok(())
}

#[allow(
    clippy::needless_range_loop,
    reason = "fixed challenge repetition indices are source-contract pinned"
)]
fn reconstruct_forced_zero_rows(
    responses: &[u8],
    hashers: &mut [DirectRelationRnsFirstMessageHasherV1; CHALLENGE_REPETITIONS_V1],
) -> Result<(), ZkAmsMkheErrorV1> {
    let mut response = try_zeroed_u64(RELEASE_RING_COEFFICIENTS_V1)?;
    for (row, slot) in GALOIS_FORCED_ROW_SLOTS_V1 {
        for (limb, modulus) in RELEASE_MODULI_V1.iter().copied().enumerate() {
            for repetition in 0..CHALLENGE_REPETITIONS_V1 {
                decode_response_slots(responses, repetition, &[slot], modulus, &mut response)?;
                absorb_residue_limb(&mut hashers[repetition], row, limb, &response)?;
            }
        }
    }
    Ok(())
}

fn finish_rns_hashers(
    hashers: &mut [DirectRelationRnsFirstMessageHasherV1; CHALLENGE_REPETITIONS_V1],
) -> Result<[[u8; 32]; CHALLENGE_REPETITIONS_V1], ZkAmsMkheErrorV1> {
    let hashers = core::mem::replace(
        hashers,
        core::array::from_fn(|_| {
            DirectRelationRnsFirstMessageHasherV1::new(PersistentDirectRelationV1::Galois)
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
        || slots.len() > GALOIS_ACTIVE_RESPONSE_SLOTS_V1.len()
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
            let encoded = responses
                .get(
                    source
                        ..source
                            .checked_add(core::mem::size_of::<i64>())
                            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                )
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

fn scatter_gadget_automorphism(
    source: &[u64],
    exponent: usize,
    gadget: u64,
    modulus: u64,
    destination: &mut [u64],
) -> Result<(), ZkAmsMkheErrorV1> {
    let degree = source.len();
    let twice_degree = degree
        .checked_mul(2)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if degree < 2
        || !degree.is_power_of_two()
        || degree > RELEASE_RING_COEFFICIENTS_V1
        || destination.len() != degree
        || exponent == 0
        || exponent >= twice_degree
        || exponent.is_multiple_of(2)
        || modulus < 3
        || gadget >= modulus
        || source.iter().any(|value| *value >= modulus)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    destination.fill(0);
    for (index, value) in source.iter().copied().enumerate() {
        let mapped = index
            .checked_mul(exponent)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            % twice_degree;
        let scaled = mod_mul(value, gadget, modulus);
        destination[mapped % degree] = if mapped >= degree && scaled != 0 {
            modulus - scaled
        } else {
            scaled
        };
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
    let mut bytes = try_zeroed_bytes(
        RELEASE_RING_COEFFICIENTS_V1
            .checked_mul(core::mem::size_of::<u64>())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )?;
    for (encoded, residue) in bytes.chunks_exact_mut(8).zip(residues) {
        encoded.copy_from_slice(&residue.to_be_bytes());
    }
    hasher.absorb_limb(row, limb, &bytes)
}

fn reconstruct_galois_b_coefficient(
    a_times_s: u64,
    gadget_times_automorphed_s: u64,
    plaintext_times_e: u64,
    challenge_times_b: u64,
    modulus: u64,
) -> u64 {
    mod_sub(
        mod_add(
            mod_sub(gadget_times_automorphed_s, a_times_s, modulus),
            plaintext_times_e,
            modulus,
        ),
        challenge_times_b,
        modulus,
    )
}

fn gadget_residue(
    digit: usize,
    gadget_base_log: u8,
    modulus: u64,
) -> Result<u64, ZkAmsMkheErrorV1> {
    if digit >= release_profile_v1().gadget_digits || gadget_base_log == 0 || modulus < 3 {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(mod_pow(
        mod_pow(2, u64::from(gadget_base_log), modulus),
        u64::try_from(digit).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        modulus,
    ))
}

fn validate_rns_live_payload_accounting() -> Result<(), ZkAmsMkheErrorV1> {
    let base = GALOIS_TARGET_A_LIMB_BYTES_V1
        .checked_add(GALOIS_B_REPLAY_LIMB_BYTES_V1)
        .and_then(|value| value.checked_add(GALOIS_RESPONSE_LIMB_BYTES_V1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let ntt = base
        .checked_add(GALOIS_NEGACYCLIC_NTT_BYTES_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let assembly = base
        .checked_add(GALOIS_NEGACYCLIC_PRODUCT_BYTES_V1)
        .and_then(|value| value.checked_add(GALOIS_ROW_OUTPUT_BYTES_V1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let hash = base
        .checked_add(GALOIS_ROW_OUTPUT_BYTES_V1)
        .and_then(|value| value.checked_add(GALOIS_CANONICAL_ENCODING_BYTES_V1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let replay = base
        .checked_add(GALOIS_B_REPLAY_SCRATCH_BYTES_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let forced = GALOIS_TARGET_A_LIMB_BYTES_V1
        .checked_add(GALOIS_CANONICAL_ENCODING_BYTES_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let peak = ntt.max(assembly).max(hash).max(replay).max(forced);
    if base != GALOIS_ROW_ZERO_BASE_BYTES_V1
        || ntt != GALOIS_ROW_ZERO_NTT_LIVE_BYTES_V1
        || assembly != GALOIS_ROW_ZERO_ASSEMBLY_LIVE_BYTES_V1
        || hash != GALOIS_ROW_ZERO_HASH_LIVE_BYTES_V1
        || replay != GALOIS_TYPED_B_REPLAY_LIVE_BYTES_V1
        || forced != GALOIS_FORCED_ROW_LIVE_BYTES_V1
        || peak != GALOIS_RNS_LIVE_PAYLOAD_CEILING_BYTES_V1
        || peak >= GALOIS_RNS_PAYLOAD_LIMIT_BYTES_V1
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
#[path = "galois_semantic_verifier_v1_tests.rs"]
mod tests;
