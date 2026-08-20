//! In-place four-challenge RKG1 response construction with whole-box retry.

use super::super::{
    ExactBindingTranscriptContextV1, MASK_COEFFICIENT_BOUND_V1, OUTER_RETRY_CEILING_V1,
    PreparedDirectRkgOneCreatorPermitV1, RESPONSE_COEFFICIENT_BOUND_V1,
    WITNESS_CHUNK_COEFFICIENTS_V1, sample_exact_uniform_signed_box,
};
use super::{
    BLIND_RESPONSE_BYTES_V1, CHALLENGE_REPETITIONS_V1, DirectRelationFirstMessageDigestsV1,
    RECONSTRUCTED_COMMITMENT_BYTES_V1, RESPONSE_BYTES_V1, WITNESS_COUNT_V1,
    challenge_vector_from_first_messages, commitment_first_message_digest,
    predecode_v1::{blind_response_offset, response_offset},
    response_commitment_v1::commit_direct_response_mask_first_message_v1,
};
use crate::vega::zk_ams::mkhe::ZkAmsMkheErrorV1;
use crate::vega::{MaskedRelaxedRandomSourceV1, VegaT256ScalarV1 as Scalar};
#[path = "rkg_one_creator_response_v1/rns_first_messages_v1.rs"]
mod rns_first_messages_v1;

const CHUNKS_PER_WITNESS_V1: usize = 8;
const MASK_BLINDING_COUNT_V1: usize =
    CHALLENGE_REPETITIONS_V1 * WITNESS_COUNT_V1 * CHUNKS_PER_WITNESS_V1;
const MASK_BLINDING_ENTROPY_BYTES_V1: usize = 64;

const _: () = {
    assert!(MASK_BLINDING_COUNT_V1 == 192);
    assert!(RESPONSE_BYTES_V1 == 25_165_824);
    assert!(BLIND_RESPONSE_BYTES_V1 == 6_144);
};

pub(super) fn create_direct_rkg_one_responses_v1<R: MaskedRelaxedRandomSourceV1>(
    permit: &PreparedDirectRkgOneCreatorPermitV1<'_>,
    transcript_context: ExactBindingTranscriptContextV1,
    responses: &mut [u8],
    blind_responses: &mut [u8],
    random: &mut R,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if responses.len() != RESPONSE_BYTES_V1 || blind_responses.len() != BLIND_RESPONSE_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    for _ in 0..OUTER_RETRY_CEILING_V1 {
        responses.fill(0);
        blind_responses.fill(0);
        fill_exact_masks_v1(responses, random)?;
        let blindings = ZeroizingMaskBlindingsV1::sample(random)?;
        let commitment_digests = commitment_first_messages_v1(responses, &blindings)?;
        let rns_digests = rns_first_messages_v1::rkg_one_rns_first_messages_v1(permit, responses)?;
        let first_messages =
            DirectRelationFirstMessageDigestsV1::new(rns_digests, commitment_digests)?;
        let (seed, challenges) =
            challenge_vector_from_first_messages(transcript_context, first_messages)?;
        if seed == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let accepted = transform_responses_in_place_v1(permit, responses, challenges)?;
        if !accepted {
            responses.fill(0);
            continue;
        }
        encode_blind_responses_v1(permit, blind_responses, challenges, &blindings)?;
        return Ok(seed);
    }
    responses.fill(0);
    blind_responses.fill(0);
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}

fn fill_exact_masks_v1<R: MaskedRelaxedRandomSourceV1>(
    responses: &mut [u8],
    random: &mut R,
) -> Result<(), ZkAmsMkheErrorV1> {
    for encoded in responses.chunks_exact_mut(8) {
        let mask = sample_exact_uniform_signed_box(random, MASK_COEFFICIENT_BOUND_V1)?;
        encoded.copy_from_slice(&mask.to_be_bytes());
    }
    Ok(())
}

fn commitment_first_messages_v1(
    responses: &[u8],
    blindings: &ZeroizingMaskBlindingsV1,
) -> Result<[[u8; 32]; CHALLENGE_REPETITIONS_V1], ZkAmsMkheErrorV1> {
    let mut digests = [[0_u8; 32]; CHALLENGE_REPETITIONS_V1];
    for (repetition, digest) in digests.iter_mut().enumerate() {
        let mut encoded = [0_u8; RECONSTRUCTED_COMMITMENT_BYTES_V1];
        let mut cursor = 0_usize;
        for slot in 0..WITNESS_COUNT_V1 {
            for chunk in 0..CHUNKS_PER_WITNESS_V1 {
                let coefficient = chunk * WITNESS_CHUNK_COEFFICIENTS_V1;
                let start = response_offset(repetition, slot, coefficient)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
                let end = start
                    .checked_add(WITNESS_CHUNK_COEFFICIENTS_V1 * 8)
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
                let masks = ZeroizingMaskChunkV1::decode(
                    responses
                        .get(start..end)
                        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                )?;
                let point = commit_direct_response_mask_first_message_v1(
                    &masks.0,
                    blindings.get(repetition, slot, chunk)?,
                )
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
                let point_end = cursor
                    .checked_add(point.as_bytes().len())
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
                encoded[cursor..point_end].copy_from_slice(point.as_bytes());
                cursor = point_end;
            }
        }
        if cursor != encoded.len() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        *digest = commitment_first_message_digest(
            super::PersistentDirectRelationV1::RkgRoundOne,
            &encoded,
        )?;
    }
    Ok(digests)
}

fn transform_responses_in_place_v1(
    permit: &PreparedDirectRkgOneCreatorPermitV1<'_>,
    responses: &mut [u8],
    challenges: [u32; CHALLENGE_REPETITIONS_V1],
) -> Result<bool, ZkAmsMkheErrorV1> {
    let mut accepted = true;
    for (repetition, challenge) in challenges.into_iter().enumerate() {
        for slot in 0..WITNESS_COUNT_V1 {
            for coefficient in 0..super::RELEASE_RING_COEFFICIENTS_V1 {
                let offset = response_offset(repetition, slot, coefficient)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
                let word = responses
                    .get_mut(offset..offset + 8)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
                let mask = i64::from_be_bytes(
                    word.try_into()
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                );
                let response =
                    permit.response_coefficient_v1(slot, coefficient, mask, challenge)?;
                accepted &= response.unsigned_abs() <= RESPONSE_COEFFICIENT_BOUND_V1 as u64;
                word.copy_from_slice(&response.to_be_bytes());
            }
        }
    }
    Ok(accepted)
}

fn encode_blind_responses_v1(
    permit: &PreparedDirectRkgOneCreatorPermitV1<'_>,
    output: &mut [u8],
    challenges: [u32; CHALLENGE_REPETITIONS_V1],
    blindings: &ZeroizingMaskBlindingsV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    for (repetition, challenge) in challenges.into_iter().enumerate() {
        for slot in 0..WITNESS_COUNT_V1 {
            for chunk in 0..CHUNKS_PER_WITNESS_V1 {
                let offset = blind_response_offset(repetition, slot, chunk)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
                let response = permit.response_blinding_v1(
                    slot,
                    chunk,
                    blindings.get(repetition, slot, chunk)?,
                    challenge,
                )?;
                output[offset..offset + 32].copy_from_slice(&response.to_be_bytes());
            }
        }
    }
    Ok(())
}

struct ZeroizingMaskBlindingsV1([Scalar; MASK_BLINDING_COUNT_V1]);

impl ZeroizingMaskBlindingsV1 {
    fn sample<R: MaskedRelaxedRandomSourceV1>(random: &mut R) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut owner = Self([Scalar::zero(); MASK_BLINDING_COUNT_V1]);
        for blinding in &mut owner.0 {
            let mut entropy =
                ZeroizingMaskBlindingEntropyV1([0_u8; MASK_BLINDING_ENTROPY_BYTES_V1]);
            random
                .fill_bytes(&mut entropy.0)
                .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
            *blinding = Scalar::from_uniform_le_bytes_ref(&entropy.0);
        }
        Ok(owner)
    }

    fn get(
        &self,
        repetition: usize,
        slot: usize,
        chunk: usize,
    ) -> Result<&Scalar, ZkAmsMkheErrorV1> {
        let index = repetition * WITNESS_COUNT_V1 * CHUNKS_PER_WITNESS_V1
            + slot * CHUNKS_PER_WITNESS_V1
            + chunk;
        self.0
            .get(index)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)
    }
}

impl Drop for ZeroizingMaskBlindingsV1 {
    fn drop(&mut self) {
        let blindings = core::hint::black_box(&mut self.0);
        for blinding in blindings.iter_mut() {
            blinding.clear_secret();
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *blindings);
    }
}

struct ZeroizingMaskBlindingEntropyV1([u8; MASK_BLINDING_ENTROPY_BYTES_V1]);

impl Drop for ZeroizingMaskBlindingEntropyV1 {
    fn drop(&mut self) {
        let bytes = core::hint::black_box(&mut self.0);
        bytes.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *bytes);
    }
}

struct ZeroizingMaskChunkV1(Vec<i64>);

impl ZeroizingMaskChunkV1 {
    fn decode(bytes: &[u8]) -> Result<Self, ZkAmsMkheErrorV1> {
        if bytes.len() != WITNESS_CHUNK_COEFFICIENTS_V1 * 8 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let mut values = Self(Vec::new());
        values
            .0
            .try_reserve_exact(WITNESS_CHUNK_COEFFICIENTS_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for word in bytes.chunks_exact(8) {
            values.0.push(i64::from_be_bytes(
                word.try_into()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            ));
        }
        Ok(values)
    }
}

impl Drop for ZeroizingMaskChunkV1 {
    fn drop(&mut self) {
        let values = core::hint::black_box(self.0.as_mut_slice());
        values.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *values);
    }
}
