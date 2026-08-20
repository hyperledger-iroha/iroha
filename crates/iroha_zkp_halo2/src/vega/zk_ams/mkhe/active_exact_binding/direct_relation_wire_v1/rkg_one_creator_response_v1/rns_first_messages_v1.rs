//! RKG1 RNS first-message digests from retained common `a` and fresh masks.

use super::super::super::PreparedDirectRkgOneCreatorPermitV1;
use super::super::{
    CHALLENGE_REPETITIONS_V1, DirectRelationRnsFirstMessageHasherV1, PersistentDirectRelationV1,
    RELEASE_RING_COEFFICIENTS_V1, RELEASE_RNS_LIMBS_V1, predecode_v1::response_offset,
};
use crate::vega::zk_ams::mkhe::{
    ZkAmsMkheErrorV1,
    collective::borrowed_product::{
        direct_rkg_one_h0_limb_from_signed_v1, direct_rkg_one_h1_limb_from_signed_v1,
    },
    manifest::{RELEASE_MODULI_V1, RELEASE_NEGACYCLIC_ROOTS_V1, release_profile_v1},
    mod_pow, signed_mod,
};

const RKG_ONE_GADGET_BASE_LOG_V1: u64 = 60;

pub(super) fn rkg_one_rns_first_messages_v1(
    permit: &PreparedDirectRkgOneCreatorPermitV1<'_>,
    masks: &[u8],
) -> Result<[[u8; 32]; CHALLENGE_REPETITIONS_V1], ZkAmsMkheErrorV1> {
    let mut hashers: [DirectRelationRnsFirstMessageHasherV1; CHALLENGE_REPETITIONS_V1] =
        core::array::from_fn(|_| {
            DirectRelationRnsFirstMessageHasherV1::new(PersistentDirectRelationV1::RkgRoundOne)
        });
    let mut decoded = zeroed_vec_v1::<i64>(3 * RELEASE_RING_COEFFICIENTS_V1)?;
    let mut encoded = zeroed_vec_v1::<u8>(8 * RELEASE_RING_COEFFICIENTS_V1)?;
    let profile = release_profile_v1();
    let digit = usize::from(permit.context().digit_index());

    for (limb, (&modulus, root)) in RELEASE_MODULI_V1
        .iter()
        .zip(RELEASE_NEGACYCLIC_ROOTS_V1)
        .enumerate()
    {
        let common_a = permit.common_a_limb_v1(limb)?;
        let gadget = gadget_residue_v1(digit, modulus)?;
        let plaintext = profile.plaintext_modulus.residue(modulus);
        for (repetition, hasher) in hashers.iter_mut().enumerate() {
            decode_mask_slots_v1(masks, repetition, &[0, 1, 2], &mut decoded.0)?;
            let (s, tail) = decoded.0.split_at(RELEASE_RING_COEFFICIENTS_V1);
            let (u, e0) = tail.split_at(RELEASE_RING_COEFFICIENTS_V1);
            let first_message = direct_rkg_one_h0_limb_from_signed_v1(
                common_a, s, u, e0, gadget, plaintext, modulus, root,
            )?;
            encode_residues_v1(&first_message, &mut encoded.0);
            hasher.absorb_limb(0, limb, &encoded.0)?;
        }
    }

    for (limb, (&modulus, root)) in RELEASE_MODULI_V1
        .iter()
        .zip(RELEASE_NEGACYCLIC_ROOTS_V1)
        .enumerate()
    {
        let common_a = permit.common_a_limb_v1(limb)?;
        let plaintext = profile.plaintext_modulus.residue(modulus);
        for (repetition, hasher) in hashers.iter_mut().enumerate() {
            decode_mask_slots_v1(
                masks,
                repetition,
                &[0, 3],
                &mut decoded.0[..2 * RELEASE_RING_COEFFICIENTS_V1],
            )?;
            let (s, e1) = decoded.0[..2 * RELEASE_RING_COEFFICIENTS_V1]
                .split_at(RELEASE_RING_COEFFICIENTS_V1);
            let first_message =
                direct_rkg_one_h1_limb_from_signed_v1(common_a, s, e1, plaintext, modulus, root)?;
            encode_residues_v1(&first_message, &mut encoded.0);
            hasher.absorb_limb(1, limb, &encoded.0)?;
        }
    }

    for (row, slot) in [(2, 4), (3, 5)] {
        for (limb, modulus) in RELEASE_MODULI_V1.iter().copied().enumerate() {
            for (repetition, hasher) in hashers.iter_mut().enumerate() {
                decode_mask_slots_v1(
                    masks,
                    repetition,
                    &[slot],
                    &mut decoded.0[..RELEASE_RING_COEFFICIENTS_V1],
                )?;
                encode_signed_residues_v1(
                    &decoded.0[..RELEASE_RING_COEFFICIENTS_V1],
                    modulus,
                    &mut encoded.0,
                );
                hasher.absorb_limb(row, limb, &encoded.0)?;
            }
        }
    }
    let mut digests = [[0_u8; 32]; CHALLENGE_REPETITIONS_V1];
    for (digest, hasher) in digests.iter_mut().zip(hashers) {
        *digest = hasher.finish()?;
    }
    Ok(digests)
}

fn decode_mask_slots_v1(
    masks: &[u8],
    repetition: usize,
    slots: &[usize],
    output: &mut [i64],
) -> Result<(), ZkAmsMkheErrorV1> {
    if output.len() != slots.len() * RELEASE_RING_COEFFICIENTS_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    for (destination, slot) in output
        .chunks_exact_mut(RELEASE_RING_COEFFICIENTS_V1)
        .zip(slots)
    {
        for (coefficient, residue) in destination.iter_mut().enumerate() {
            let offset = response_offset(repetition, *slot, coefficient)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            *residue = i64::from_be_bytes(
                masks[offset..offset + 8]
                    .try_into()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            );
        }
    }
    Ok(())
}

fn encode_residues_v1(residues: &[u64], encoded: &mut [u8]) {
    for (destination, residue) in encoded.chunks_exact_mut(8).zip(residues) {
        destination.copy_from_slice(&residue.to_be_bytes());
    }
}

fn encode_signed_residues_v1(residues: &[i64], modulus: u64, encoded: &mut [u8]) {
    for (destination, residue) in encoded.chunks_exact_mut(8).zip(residues) {
        destination.copy_from_slice(&signed_mod(*residue, modulus).to_be_bytes());
    }
}

fn gadget_residue_v1(digit: usize, modulus: u64) -> Result<u64, ZkAmsMkheErrorV1> {
    if digit >= RELEASE_RNS_LIMBS_V1 || modulus < 3 {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(mod_pow(
        mod_pow(2, RKG_ONE_GADGET_BASE_LOG_V1, modulus),
        u64::try_from(digit).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        modulus,
    ))
}

struct ZeroizingVecV1<T: Copy + Default>(Vec<T>);

impl<T: Copy + Default> Drop for ZeroizingVecV1<T> {
    fn drop(&mut self) {
        let values = core::hint::black_box(self.0.as_mut_slice());
        values.fill(T::default());
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *values);
    }
}

fn zeroed_vec_v1<T: Copy + Default>(length: usize) -> Result<ZeroizingVecV1<T>, ZkAmsMkheErrorV1> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    output.resize(length, T::default());
    Ok(ZeroizingVecV1(output))
}
