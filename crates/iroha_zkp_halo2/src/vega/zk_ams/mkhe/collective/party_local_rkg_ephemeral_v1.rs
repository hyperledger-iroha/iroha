//! State-owned, authority-neutral creator that retains `u_i` and returns public membership only.

use super::{
    super::{
        MAX_RANDOM_REJECTION_ATTEMPTS_V1, SecretPolynomial, ZkAmsMkheErrorV1,
        active_exact_binding::{
            PersistentWitnessConsumerV1, VerifiedPersistentWitnessBindingSetV1,
        },
        direct_collective_eval_ceremony::{
            ZkAmsMkheDirectCeremonyContextV1, ZkAmsMkheDirectEvaluatedKeyTargetV1,
        },
        direct_rkg_ephemeral_membership::{
            ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
            ZkAmsMkheDirectRkgEphemeralMembershipErrorV1,
            ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1,
        },
        exact_eight_chunk_membership::{
            ExactEightChunkMembershipErrorV1, ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1,
            ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1,
            ZK_AMS_MKHE_RKG_EPHEMERAL_MEMBERSHIP_WIRE_BYTES_V1,
        },
        manifest::{ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, release_profile_v1},
    },
    ZkAmsMkheCollectivePartyStateV1, clear_secret_i64_slice_v1,
};
use crate::{
    generalized_bulletproof::{GeneralizedBulletproofErrorV1, ProofRandomSource},
    vega::{
        MaskedRelaxedRandomSourceV1, VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
        bulletproof_t256::{
            ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1, ZkAmsT256MembershipBoundV1,
            ZkAmsT256MembershipErrorV1, commit_zk_ams_t256_membership_chunk_v1,
        },
    },
};

#[path = "direct_rkg_one_candidate_v1.rs"]
mod direct_rkg_one_candidate_v1;
#[path = "direct_rkg_one_publication_v1.rs"]
mod direct_rkg_one_publication_v1;
#[path = "direct_rkg_one_sealed_candidate_v1.rs"]
mod direct_rkg_one_sealed_candidate_v1;
pub(in crate::vega::zk_ams::mkhe) use direct_rkg_one_candidate_v1::DirectRkgOneProverSessionV1;
pub(in crate::vega::zk_ams::mkhe) use direct_rkg_one_publication_v1::DirectRkgOnePublicationOwnerV1;

const RKG_EPHEMERAL_BLINDING_ENTROPY_BYTES_V1: usize = 64;
const RKG_EPHEMERAL_POINT_WIRE_BYTES_V1: usize = 33;
const RKG_EPHEMERAL_CREATION_MASK_BITS_V1: usize = 38;
type RkgEphemeralCommitmentWireV1 =
    [[u8; RKG_EPHEMERAL_POINT_WIRE_BYTES_V1]; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1];
const RKG_EPHEMERAL_RETAINED_PAYLOAD_BYTES_V1: usize = ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1
    * core::mem::size_of::<i64>()
    + ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1 * core::mem::size_of::<Scalar>()
    + core::mem::size_of::<RkgEphemeralCommitmentWireV1>();
const RKG_EPHEMERAL_WITH_NARROWING_BYTES_V1: usize = RKG_EPHEMERAL_RETAINED_PAYLOAD_BYTES_V1
    + ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1 * core::mem::size_of::<i8>();

const _: () = {
    assert!(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 == 8);
    assert!(ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1 == 8);
    assert!(ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1 == 131_072);
    assert!(RKG_EPHEMERAL_CREATION_MASK_BITS_V1 <= u64::BITS as usize);
    assert!(core::mem::size_of::<RkgEphemeralCommitmentWireV1>() == 264);
    assert!(ZK_AMS_MKHE_RKG_EPHEMERAL_MEMBERSHIP_WIRE_BYTES_V1 == 12_291);
    assert!(RKG_EPHEMERAL_RETAINED_PAYLOAD_BYTES_V1 == 1_049_096);
    assert!(RKG_EPHEMERAL_WITH_NARROWING_BYTES_V1 == 1_180_168);
};

/// Public membership evidence only; carries no binding or release authority.
#[allow(dead_code)]
pub(super) struct StateOwnedDirectRkgEphemeralMembershipPrecursorV1 {
    membership: ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1,
}

/// Sole owner; candidate creation consumes this slot and never reinserts it.
#[allow(dead_code)]
pub(super) struct PartyLocalRkgEphemeralOpeningV1 {
    context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
    u: RkgEphemeralSecretV1,
    blindings: RkgEphemeralCommitmentBlindingsV1,
    retained_commitment_wire: RkgEphemeralCommitmentWireV1,
}

struct RkgEphemeralSecretV1(SecretPolynomial);
impl Drop for RkgEphemeralSecretV1 {
    fn drop(&mut self) {
        clear_secret_i64_slice_v1(&mut self.0.coefficients);
        #[cfg(test)]
        tests::record_drop_v1(0, self.0.coefficients.iter().all(|value| *value == 0));
    }
}

struct ZeroizingRkgEphemeralBlindingEntropyV1([u8; RKG_EPHEMERAL_BLINDING_ENTROPY_BYTES_V1]);

impl Drop for ZeroizingRkgEphemeralBlindingEntropyV1 {
    fn drop(&mut self) {
        let bytes = core::hint::black_box(&mut self.0);
        bytes.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *bytes);
        #[cfg(test)]
        tests::record_drop_v1(1, bytes.iter().all(|value| *value == 0));
    }
}

struct RkgEphemeralCommitmentBlindingsV1([Scalar; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1]);

impl RkgEphemeralCommitmentBlindingsV1 {
    fn sample<R: MaskedRelaxedRandomSourceV1>(random: &mut R) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut owner = Self([Scalar::zero(); ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1]);
        for blinding in &mut owner.0 {
            let mut accepted = false;
            for _ in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
                let mut entropy = ZeroizingRkgEphemeralBlindingEntropyV1(
                    [0; RKG_EPHEMERAL_BLINDING_ENTROPY_BYTES_V1],
                );
                random
                    .fill_bytes(&mut entropy.0)
                    .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
                *blinding = Scalar::from_uniform_le_bytes_ref(&entropy.0);
                if !blinding.is_zero() {
                    accepted = true;
                    break;
                }
            }
            if !accepted {
                return Err(ZkAmsMkheErrorV1::RandomUnavailable);
            }
        }
        Ok(owner)
    }
}

impl Drop for RkgEphemeralCommitmentBlindingsV1 {
    fn drop(&mut self) {
        let blindings = core::hint::black_box(&mut self.0);
        for blinding in blindings.iter_mut() {
            blinding.clear_secret();
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *blindings);
        #[cfg(test)]
        tests::record_drop_v1(2, blindings.iter().all(|value| value.is_zero()));
    }
}

struct ZeroizingRkgEphemeralCoefficientsV1(Vec<i8>);

impl ZeroizingRkgEphemeralCoefficientsV1 {
    fn from_ternary_secret(secret: &SecretPolynomial) -> Result<Self, ZkAmsMkheErrorV1> {
        if secret.coefficients.len() != ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut coefficients = Self(Vec::new());
        coefficients
            .0
            .try_reserve_exact(ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for coefficient in secret.coefficients.iter().copied() {
            let coefficient =
                i8::try_from(coefficient).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
            if !(-1..=1).contains(&coefficient) {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            coefficients.0.push(coefficient);
        }
        Ok(coefficients)
    }
}

impl Drop for ZeroizingRkgEphemeralCoefficientsV1 {
    fn drop(&mut self) {
        let coefficients = core::hint::black_box(&mut self.0);
        coefficients.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *coefficients);
        #[cfg(test)]
        tests::record_drop_v1(3, coefficients.iter().all(|value| *value == 0));
    }
}

impl ZkAmsMkheCollectivePartyStateV1 {
    /// Prepare public membership while retaining the sole party-local opening.
    #[allow(dead_code)]
    pub(super) fn prepare_state_owned_direct_rkg_ephemeral_membership_v1<
        R: MaskedRelaxedRandomSourceV1 + ProofRandomSource,
    >(
        &mut self,
        roster: &super::super::active::ZkAmsMkheGovernedActiveRosterV1,
        bindings: &VerifiedPersistentWitnessBindingSetV1,
        digit_index: usize,
        random: &mut R,
    ) -> Result<StateOwnedDirectRkgEphemeralMembershipPrecursorV1, ZkAmsMkheErrorV1> {
        roster.validate()?;
        bindings.validate_for_consumer(roster, PersistentWitnessConsumerV1::RkgRoundOne)?;
        let profile = release_profile_v1();
        profile.validate()?;
        if profile.ring_degree != ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1
            || profile.gadget_digits != RKG_EPHEMERAL_CREATION_MASK_BITS_V1
            || digit_index >= profile.gadget_digits
            || self.party_local_rkg_ephemeral_opening.is_some()
            || self.party_local_rkg_ephemeral_creation_mask >> RKG_EPHEMERAL_CREATION_MASK_BITS_V1
                != 0
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let digit_bit = 1_u64
            .checked_shl(
                u32::try_from(digit_index)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if self.party_local_rkg_ephemeral_creation_mask & digit_bit != 0 {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let party_index = usize::from(self.party_index());
        if party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPartySet);
        }
        let cached_binding =
            self.persistent_secret_binding_for(roster, PersistentWitnessConsumerV1::RkgRoundOne)?;
        if cached_binding.identity_digest() != bindings.identity_digests()[party_index] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let state_commitments = self.recompute_persistent_direct_commitments_v1()?;
        if cached_binding.commitments() != &state_commitments {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let direct_context = ZkAmsMkheDirectCeremonyContextV1::from_verified_binding_set(
            roster,
            bindings,
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization,
            digit_index,
        )?;
        let context = ZkAmsMkheDirectRkgEphemeralMembershipContextV1::from_verified_binding_set(
            roster,
            bindings,
            &direct_context,
            party_index,
        )?;

        let u = sample_nonzero_rkg_ephemeral_v1(&profile, random)?;
        let blindings = RkgEphemeralCommitmentBlindingsV1::sample(random)?;
        let coefficients = ZeroizingRkgEphemeralCoefficientsV1::from_ternary_secret(&u.0)?;
        let commitments = commit_rkg_ephemeral_opening_v1(&coefficients.0, &blindings.0)?;
        let retained_commitment_wire = encode_rkg_ephemeral_commitments_v1(&commitments)?;
        let membership =
            prove_rkg_ephemeral_membership_v1(context, &coefficients.0, &blindings.0, random)
                .map_err(map_membership_error_v1)?;
        let evidence_commitment_wire =
            encode_rkg_ephemeral_commitments_v1(&membership.commitments())?;
        if membership.context() != context || evidence_commitment_wire != retained_commitment_wire {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }

        let owner = PartyLocalRkgEphemeralOpeningV1 {
            context,
            u,
            blindings,
            retained_commitment_wire,
        };
        let precursor = StateOwnedDirectRkgEphemeralMembershipPrecursorV1 { membership };
        self.party_local_rkg_ephemeral_creation_mask |= digit_bit;
        self.party_local_rkg_ephemeral_opening = Some(owner);
        Ok(precursor)
    }
}

fn sample_nonzero_rkg_ephemeral_v1<R: MaskedRelaxedRandomSourceV1>(
    profile: &super::super::BgvProfile,
    random: &mut R,
) -> Result<RkgEphemeralSecretV1, ZkAmsMkheErrorV1> {
    for _ in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
        let candidate = RkgEphemeralSecretV1(SecretPolynomial::sample_ternary(profile, random)?);
        if candidate.0.coefficients.iter().any(|&value| value != 0) {
            return Ok(candidate);
        }
    }
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}

fn prove_rkg_ephemeral_membership_v1<R: ProofRandomSource>(
    context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
    coefficients: &[i8],
    blindings: &[Scalar; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
    random: &mut R,
) -> Result<
    ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1,
    ZkAmsMkheDirectRkgEphemeralMembershipErrorV1,
> {
    #[cfg(test)]
    if let Some(injected) = tests::injected_membership_v1(context, coefficients, blindings, random)
    {
        return injected;
    }
    ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1::prove(context, coefficients, blindings, random)
}

fn commit_rkg_ephemeral_opening_v1(
    coefficients: &[i8],
    blindings: &[Scalar; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
) -> Result<[Point; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1], ZkAmsMkheErrorV1> {
    let chunks = coefficients.chunks_exact(ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1);
    if !chunks.remainder().is_empty() || chunks.len() != blindings.len() {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut commitments = Vec::new();
    commitments
        .try_reserve_exact(blindings.len())
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    for (chunk, blinding) in chunks.zip(blindings.iter()) {
        commitments.push(
            commit_zk_ams_t256_membership_chunk_v1(
                ZkAmsT256MembershipBoundV1::One,
                chunk,
                blinding,
            )
            .map_err(map_t256_error_v1)?,
        );
    }
    commitments
        .try_into()
        .map_err(|_: Vec<Point>| ZkAmsMkheErrorV1::InvalidKeyMaterial)
}

fn encode_rkg_ephemeral_commitments_v1(
    commitments: &[Point; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
) -> Result<RkgEphemeralCommitmentWireV1, ZkAmsMkheErrorV1> {
    let mut wire =
        [[0_u8; RKG_EPHEMERAL_POINT_WIRE_BYTES_V1]; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1];
    for (commitment, destination) in commitments.iter().zip(wire.iter_mut()) {
        commitment
            .write_non_identity_wire_bytes_ref(destination)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    }
    Ok(wire)
}

fn map_membership_error_v1(
    error: ZkAmsMkheDirectRkgEphemeralMembershipErrorV1,
) -> ZkAmsMkheErrorV1 {
    match error {
        ZkAmsMkheDirectRkgEphemeralMembershipErrorV1::ExactMembership(
            ExactEightChunkMembershipErrorV1::Membership(error),
        ) => map_t256_error_v1(error),
        ZkAmsMkheDirectRkgEphemeralMembershipErrorV1::Context
        | ZkAmsMkheDirectRkgEphemeralMembershipErrorV1::ExactMembership(_) => {
            ZkAmsMkheErrorV1::InvalidKeyMaterial
        }
    }
}

fn map_t256_error_v1(error: ZkAmsT256MembershipErrorV1) -> ZkAmsMkheErrorV1 {
    match error {
        ZkAmsT256MembershipErrorV1::Backend(
            GeneralizedBulletproofErrorV1::RandomnessUnavailable
            | GeneralizedBulletproofErrorV1::ProverRandomnessExhausted,
        ) => ZkAmsMkheErrorV1::RandomUnavailable,
        ZkAmsT256MembershipErrorV1::Backend(GeneralizedBulletproofErrorV1::ResourceOverflow) => {
            ZkAmsMkheErrorV1::ResourceCeilingExceeded
        }
        _ => ZkAmsMkheErrorV1::InvalidKeyMaterial,
    }
}

#[cfg(test)]
#[path = "party_local_rkg_ephemeral_v1_tests.rs"]
mod tests;
