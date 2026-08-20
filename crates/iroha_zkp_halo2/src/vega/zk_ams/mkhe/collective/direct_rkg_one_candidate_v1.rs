//! Consuming party-local ownership for one authority-neutral RKG1 candidate.

use super::super::super::{
    SecretPolynomial, ZkAmsMkheErrorV1,
    active::ZkAmsMkheGovernedActiveRosterV1,
    active_exact_binding::{PersistentWitnessConsumerV1, VerifiedPersistentWitnessBindingSetV1},
    direct_collective_eval_ceremony::ZkAmsMkheDirectCeremonyContextV1,
    direct_rkg_ephemeral_membership::ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
    exact_eight_chunk_membership::{
        DirectRelationBoundOneMembershipRoleV1, DirectRelationBoundTwoMembershipRoleV1,
        ExactEightChunkMembershipContextV1, ExactEightChunkMembershipErrorV1,
        ExactEightChunkMembershipEvidenceV1, ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1,
        ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1,
    },
    manifest::{
        RELEASE_MODULI_V1, RELEASE_NEGACYCLIC_ROOTS_V1, ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1,
        release_profile_v1,
    },
    mod_pow,
};
use super::super::ZkAmsMkheCollectivePartyStateV1;
use super::super::borrowed_product::{
    direct_rkg_one_h0_limb_from_signed_v1, direct_rkg_one_h1_limb_from_signed_v1,
};
use super::super::persistent_direct_opening_v1::PostCpkPersistentDirectOpeningGuardV1;
use super::{
    PartyLocalRkgEphemeralOpeningV1, RkgEphemeralCommitmentBlindingsV1,
    StateOwnedDirectRkgEphemeralMembershipPrecursorV1, ZeroizingRkgEphemeralCoefficientsV1,
    encode_rkg_ephemeral_commitments_v1,
};
use crate::{
    generalized_bulletproof::ProofRandomSource,
    vega::{MaskedRelaxedRandomSourceV1, VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar},
};
const RKG_ONE_GADGET_BASE_LOG_V1: u64 = 60;
const RKG_ONE_LIMBS_V1: usize = 38;
const RKG_ONE_WITNESS_SLOTS_V1: usize = 6;
const RKG_ONE_RING_DEGREE_V1: usize = 131_072;
const RKG_ONE_RETAINED_COMMON_A_BYTES_V1: usize =
    RKG_ONE_LIMBS_V1 * RKG_ONE_RING_DEGREE_V1 * core::mem::size_of::<u64>();
const _: () = {
    assert!(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 == 8);
    assert!(ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1 == 8);
    assert!(ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1 == RKG_ONE_RING_DEGREE_V1);
    assert!(RELEASE_MODULI_V1.len() == RKG_ONE_LIMBS_V1);
    assert!(RKG_ONE_RETAINED_COMMON_A_BYTES_V1 == 39_845_888);
};
struct ZeroizingDirectRkgOneCoefficientsV1(Vec<i8>);
impl ZeroizingDirectRkgOneCoefficientsV1 {
    fn from_error(secret: &SecretPolynomial) -> Result<Self, ZkAmsMkheErrorV1> {
        if secret.coefficients.len() != RKG_ONE_RING_DEGREE_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut owner = Self(Vec::new());
        owner
            .0
            .try_reserve_exact(RKG_ONE_RING_DEGREE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for value in secret.coefficients.iter().copied() {
            let value = i8::try_from(value).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
            if !(-2..=2).contains(&value) {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            owner.0.push(value);
        }
        Ok(owner)
    }
    fn zeros() -> Result<Self, ZkAmsMkheErrorV1> {
        let mut owner = Self(Vec::new());
        owner
            .0
            .try_reserve_exact(RKG_ONE_RING_DEGREE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        owner.0.resize(RKG_ONE_RING_DEGREE_V1, 0);
        Ok(owner)
    }
}
impl Drop for ZeroizingDirectRkgOneCoefficientsV1 {
    fn drop(&mut self) {
        let coefficients = core::hint::black_box(self.0.as_mut_slice());
        coefficients.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *coefficients);
    }
}
/// Move-only prover session; not creator provenance, extractor evidence, receipt,
/// binding, admission, or verifier authority.
pub(in crate::vega::zk_ams::mkhe) struct DirectRkgOneProverSessionV1<'a> {
    persistent_guard: PostCpkPersistentDirectOpeningGuardV1<'a>,
    ephemeral_owner: PartyLocalRkgEphemeralOpeningV1,
    _original_wrapper: StateOwnedDirectRkgEphemeralMembershipPrecursorV1,
    error_zero: SecretPolynomial,
    error_one: SecretPolynomial,
    bound_two_blindings: [RkgEphemeralCommitmentBlindingsV1; RKG_ONE_WITNESS_SLOTS_V1 - 2],
    persistent_commitments: [Point; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
    ephemeral_commitments: [Point; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
    // Authenticated A is still needed for A*y after both consuming replays.
    // This retains exactly 38 * 131_072 * 8 = 39_845_888 bytes.
    common_a_matrix: Vec<u64>,
}
impl<'a> DirectRkgOneProverSessionV1<'a> {
    pub(in crate::vega::zk_ams::mkhe) fn context(
        &self,
    ) -> ZkAmsMkheDirectRkgEphemeralMembershipContextV1 {
        self.ephemeral_owner.context
    }
    pub(in crate::vega::zk_ams::mkhe) const fn persistent_commitments(
        &self,
    ) -> &[Point; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1] {
        &self.persistent_commitments
    }
    pub(in crate::vega::zk_ams::mkhe) const fn ephemeral_commitments(
        &self,
    ) -> &[Point; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1] {
        &self.ephemeral_commitments
    }
    pub(in crate::vega::zk_ams::mkhe) fn prove_bound_one_v1<R: ProofRandomSource>(
        &self,
        slot: usize,
        context: ExactEightChunkMembershipContextV1<DirectRelationBoundOneMembershipRoleV1>,
        random: &mut R,
    ) -> Result<
        ExactEightChunkMembershipEvidenceV1<DirectRelationBoundOneMembershipRoleV1>,
        ZkAmsMkheErrorV1,
    > {
        match slot {
            0 => ExactEightChunkMembershipEvidenceV1::prove(
                context,
                self.persistent_guard.coefficients.as_slice(),
                self.persistent_guard.owner.blindings.as_array(),
                random,
            ),
            1 => {
                let coefficients = ZeroizingRkgEphemeralCoefficientsV1::from_ternary_secret(
                    &self.ephemeral_owner.u.0,
                )?;
                ExactEightChunkMembershipEvidenceV1::prove(
                    context,
                    &coefficients.0,
                    &self.ephemeral_owner.blindings.0,
                    random,
                )
            }
            _ => return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
        }
        .map_err(map_exact_membership_error_v1)
    }
    pub(in crate::vega::zk_ams::mkhe) fn prove_bound_two_v1<R: ProofRandomSource>(
        &self,
        slot: usize,
        context: ExactEightChunkMembershipContextV1<DirectRelationBoundTwoMembershipRoleV1>,
        random: &mut R,
    ) -> Result<
        ExactEightChunkMembershipEvidenceV1<DirectRelationBoundTwoMembershipRoleV1>,
        ZkAmsMkheErrorV1,
    > {
        let coefficients = match slot {
            2 => ZeroizingDirectRkgOneCoefficientsV1::from_error(&self.error_zero)?,
            3 => ZeroizingDirectRkgOneCoefficientsV1::from_error(&self.error_one)?,
            4 | 5 => ZeroizingDirectRkgOneCoefficientsV1::zeros()?,
            _ => return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
        };
        ExactEightChunkMembershipEvidenceV1::prove(
            context,
            &coefficients.0,
            &self.bound_two_blindings[slot - 2].0,
            random,
        )
        .map_err(map_exact_membership_error_v1)
    }
    pub(in crate::vega::zk_ams::mkhe) fn response_coefficient_v1(
        &self,
        slot: usize,
        coefficient: usize,
        mask: i64,
        challenge: u32,
    ) -> Result<i64, ZkAmsMkheErrorV1> {
        if coefficient >= RKG_ONE_RING_DEGREE_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let witness = match slot {
            0 => self
                .persistent_guard
                .owner
                .secret
                .coefficients
                .get(coefficient)
                .copied(),
            1 => self
                .ephemeral_owner
                .u
                .0
                .coefficients
                .get(coefficient)
                .copied(),
            2 => self.error_zero.coefficients.get(coefficient).copied(),
            3 => self.error_one.coefficients.get(coefficient).copied(),
            4 | 5 => Some(0),
            _ => None,
        }
        .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        mask.checked_add(
            i64::from(challenge)
                .checked_mul(witness)
                .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        )
        .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    }
    pub(in crate::vega::zk_ams::mkhe) fn response_blinding_v1(
        &self,
        slot: usize,
        chunk: usize,
        mask_blinding: &Scalar,
        challenge: u32,
    ) -> Result<Scalar, ZkAmsMkheErrorV1> {
        let witness = match slot {
            0 => self.persistent_guard.owner.blindings.as_array().get(chunk),
            1 => self.ephemeral_owner.blindings.0.get(chunk),
            2..=5 => self.bound_two_blindings[slot - 2].0.get(chunk),
            _ => None,
        }
        .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        Ok(*mask_blinding + Scalar::from_u64(u64::from(challenge)) * *witness)
    }
    pub(in crate::vega::zk_ams::mkhe) fn retain_common_a_limb_v1(
        &mut self,
        limb: usize,
        values: &[u64],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if limb >= RKG_ONE_LIMBS_V1
            || values.len() != RKG_ONE_RING_DEGREE_V1
            || self.common_a_matrix.len() != limb * RKG_ONE_RING_DEGREE_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        self.common_a_matrix
            .try_reserve_exact(values.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        self.common_a_matrix.extend_from_slice(values);
        Ok(())
    }
    pub(in crate::vega::zk_ams::mkhe) fn common_a_limb_v1(
        &self,
        limb: usize,
    ) -> Result<&[u64], ZkAmsMkheErrorV1> {
        let start = limb
            .checked_mul(RKG_ONE_RING_DEGREE_V1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let end = start
            .checked_add(RKG_ONE_RING_DEGREE_V1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        self.common_a_matrix
            .get(start..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)
    }
    pub(in crate::vega::zk_ams::mkhe) fn relation_limb_v1(
        &self,
        role: u8,
        limb: usize,
        common_a: &[u64],
    ) -> Result<Vec<u64>, ZkAmsMkheErrorV1> {
        let modulus = *RELEASE_MODULI_V1
            .get(limb)
            .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
        let root = *RELEASE_NEGACYCLIC_ROOTS_V1
            .get(limb)
            .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
        if common_a.len() != RKG_ONE_RING_DEGREE_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        let profile = release_profile_v1();
        let plaintext = profile.plaintext_modulus.residue(modulus);
        let gadget = mod_pow(
            mod_pow(2, RKG_ONE_GADGET_BASE_LOG_V1, modulus),
            u64::from(self.ephemeral_owner.context.digit_index()),
            modulus,
        );
        match role {
            0 => direct_rkg_one_h0_limb_from_signed_v1(
                common_a,
                &self.persistent_guard.owner.secret.coefficients,
                &self.ephemeral_owner.u.0.coefficients,
                &self.error_zero.coefficients,
                gadget,
                plaintext,
                modulus,
                root,
            ),
            1 => direct_rkg_one_h1_limb_from_signed_v1(
                common_a,
                &self.persistent_guard.owner.secret.coefficients,
                &self.error_one.coefficients,
                plaintext,
                modulus,
                root,
            ),
            _ => Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
        }
    }
    pub(in crate::vega::zk_ams::mkhe) fn into_compacted_post_seal_v1(self) -> impl Sized + 'a {
        let retained = (
            self.persistent_guard.into_compacted_post_seal_v1(),
            self.ephemeral_owner,
            self._original_wrapper,
            self.bound_two_blindings,
            self.persistent_commitments,
            self.ephemeral_commitments,
        );
        drop((self.error_zero, self.error_one, self.common_a_matrix));
        retained
    }
}
pub(super) fn take_ready_direct_rkg_one_prover_session_v1<'a, R>(
    state: &'a mut ZkAmsMkheCollectivePartyStateV1,
    wrapper: StateOwnedDirectRkgEphemeralMembershipPrecursorV1,
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    bindings: &VerifiedPersistentWitnessBindingSetV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
    random: &mut R,
) -> Result<DirectRkgOneProverSessionV1<'a>, ZkAmsMkheErrorV1>
where
    R: MaskedRelaxedRandomSourceV1 + ProofRandomSource,
{
    roster.validate()?;
    bindings.validate_for_consumer(roster, PersistentWitnessConsumerV1::RkgRoundOne)?;
    context.validate_rkg_ephemeral_membership_axes(roster, bindings)?;
    let party_index = usize::from(state.party_index());
    let expected_context =
        ZkAmsMkheDirectRkgEphemeralMembershipContextV1::from_verified_binding_set(
            roster,
            bindings,
            &context,
            party_index,
        )?;
    let binding =
        state.persistent_secret_binding_for(roster, PersistentWitnessConsumerV1::RkgRoundOne)?;
    let binding_identity = binding.identity_digest();
    if binding_identity != bindings.identity_digests()[party_index] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let ephemeral_owner = state
        .party_local_rkg_ephemeral_opening
        .take()
        .ok_or(ZkAmsMkheErrorV1::ReleaseUnavailable)?;
    if ephemeral_owner.context != expected_context
        || wrapper.membership.context() != expected_context
        || wrapper.membership.commitments() != ephemeral_owner_commitments_v1(&ephemeral_owner)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let digit_bit = 1_u64
        .checked_shl(u32::from(context.digit_index()))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if state.party_local_rkg_ephemeral_creation_mask & digit_bit == 0 {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let error_zero = SecretPolynomial::sample_error(&release_profile_v1(), random)?;
    let error_one = SecretPolynomial::sample_error(&release_profile_v1(), random)?;
    let bound_two_blindings = [
        RkgEphemeralCommitmentBlindingsV1::sample(random)?,
        RkgEphemeralCommitmentBlindingsV1::sample(random)?,
        RkgEphemeralCommitmentBlindingsV1::sample(random)?,
        RkgEphemeralCommitmentBlindingsV1::sample(random)?,
    ];
    let persistent_guard = PostCpkPersistentDirectOpeningGuardV1::from_installed_binding_v1(
        &mut state.persistent_direct_opening,
        &state.public_error,
        (&state.party_local_rkg_ephemeral_creation_mask, digit_bit),
    )?;
    let persistent_commitments = *persistent_guard.checked_commitments_v1()?;
    let ephemeral_commitments = ephemeral_owner_commitments_v1(&ephemeral_owner)?;
    Ok(DirectRkgOneProverSessionV1 {
        persistent_guard,
        ephemeral_owner,
        _original_wrapper: wrapper,
        error_zero,
        error_one,
        bound_two_blindings,
        persistent_commitments,
        ephemeral_commitments,
        common_a_matrix: Vec::new(),
    })
}
fn ephemeral_owner_commitments_v1(
    owner: &PartyLocalRkgEphemeralOpeningV1,
) -> Result<[Point; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1], ZkAmsMkheErrorV1> {
    let coefficients = ZeroizingRkgEphemeralCoefficientsV1::from_ternary_secret(&owner.u.0)?;
    let commitments = super::commit_rkg_ephemeral_opening_v1(&coefficients.0, &owner.blindings.0)?;
    if encode_rkg_ephemeral_commitments_v1(&commitments)? != owner.retained_commitment_wire {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(commitments)
}

fn map_exact_membership_error_v1(error: ExactEightChunkMembershipErrorV1) -> ZkAmsMkheErrorV1 {
    match error {
        ExactEightChunkMembershipErrorV1::Membership(error) => super::map_t256_error_v1(error),
        _ => ZkAmsMkheErrorV1::InvalidKeyMaterial,
    }
}

#[cfg(test)]
#[path = "direct_rkg_one_candidate_v1/direct_rkg_one_candidate_v1_tests.rs"]
mod tests;
