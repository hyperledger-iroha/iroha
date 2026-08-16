//! Once-validated, budgeted common-`a` derivation shared by staged workers.
use super::{
    ACTIVE_COLLECTIVE_PUBLIC_A_DOMAIN_V1, BgvProfile, MAX_RANDOM_REJECTION_ATTEMPTS_V1,
    Shake256Reader, ZkAmsMkheCpkRelationErrorV1, ZkAmsMkheGovernedActiveRosterV1,
    active_collective_public_a_context_v1,
};
use crate::generalized_bulletproof::try_exact_capacity_vec_v1;
const ACTIVE_COLLECTIVE_PUBLIC_A_CONTEXT_BYTES_V1: usize = 1 + 32 + 8 + 32;
const ACTIVE_COLLECTIVE_PUBLIC_A_PREFIX_BYTES_V1: usize =
    active_collective_public_a_limb_frame_bytes_v1() - 2;
/// Opaque, non-cloneable common-`a` frame authority validated once per worker.
pub(in super::super) struct ZkAmsMkhePreparedCollectivePublicAContextV1 {
    profile: BgvProfile,
    _profile_digest: [u8; 32],
    _roster_digest: [u8; 32],
    _roster_key_material_digest: [u8; 32],
    _epoch: u64,
    _cpk_transcript_digest: [u8; 32],
    frame_prefix: [u8; ACTIVE_COLLECTIVE_PUBLIC_A_PREFIX_BYTES_V1],
}
/// Validate the complete immutable profile/roster/transcript axes once and
/// freeze the native frame prefix used by every subsequent limb.
pub(in super::super) fn prepare_active_collective_public_a_v1(
    profile: &BgvProfile,
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    cpk_transcript_digest: [u8; 32],
) -> Result<ZkAmsMkhePreparedCollectivePublicAContextV1, ZkAmsMkheCpkRelationErrorV1> {
    let profile_digest = profile
        .digest()
        .map_err(|_| ZkAmsMkheCpkRelationErrorV1::GovernedContext)?;
    validate_profile_digest_axis_v1(profile_digest, roster.profile_digest())?;
    let context = active_collective_public_a_context_v1(roster, cpk_transcript_digest)?;
    if context.len() != ACTIVE_COLLECTIVE_PUBLIC_A_CONTEXT_BYTES_V1 {
        return Err(ZkAmsMkheCpkRelationErrorV1::GovernedContext);
    }
    let context_len = u32::try_from(context.len())
        .map_err(|_| ZkAmsMkheCpkRelationErrorV1::ResourceCeiling)?
        .to_be_bytes();
    let mut frame_prefix = [0_u8; ACTIVE_COLLECTIVE_PUBLIC_A_PREFIX_BYTES_V1];
    let mut cursor = 0;
    for bytes in [
        ACTIVE_COLLECTIVE_PUBLIC_A_DOMAIN_V1,
        profile_digest.as_slice(),
        context_len.as_slice(),
        context.as_slice(),
    ] {
        let end = cursor + bytes.len();
        frame_prefix[cursor..end].copy_from_slice(bytes);
        cursor = end;
    }
    if cursor != frame_prefix.len() {
        return Err(ZkAmsMkheCpkRelationErrorV1::ResourceCeiling);
    }
    Ok(ZkAmsMkhePreparedCollectivePublicAContextV1 {
        profile: profile.clone(),
        _profile_digest: profile_digest,
        _roster_digest: roster.roster_digest(),
        _roster_key_material_digest: roster.key_material_digest(),
        _epoch: roster.epoch(),
        _cpk_transcript_digest: cpk_transcript_digest,
        frame_prefix,
    })
}
/// Exact bytes cloned and absorbed for one limb, including the limb index.
pub(in super::super) const fn active_collective_public_a_limb_frame_bytes_v1() -> usize {
    ACTIVE_COLLECTIVE_PUBLIC_A_DOMAIN_V1.len()
        + 32
        + 4
        + ACTIVE_COLLECTIVE_PUBLIC_A_CONTEXT_BYTES_V1
        + 2
}
/// Compatibility path preserving the native whole-polynomial derivation.
pub(in super::super) fn derive_active_collective_public_a_limb_v1(
    profile: &BgvProfile,
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    cpk_transcript_digest: [u8; 32],
    limb: usize,
) -> Result<Vec<u64>, ZkAmsMkheCpkRelationErrorV1> {
    prepare_active_collective_public_a_v1(profile, roster, cpk_transcript_digest)?
        .derive_limb_inner_v1(limb, None)
}
impl ZkAmsMkhePreparedCollectivePublicAContextV1 {
    /// Derive one byte-identical native limb while charging every accepted or
    /// rejected SHAKE candidate to the shared whole-worker budget.
    pub(in super::super) fn derive_limb_budgeted_v1(
        &self,
        limb: usize,
        remaining_candidates: &mut u64,
    ) -> Result<Vec<u64>, ZkAmsMkheCpkRelationErrorV1> {
        self.derive_limb_inner_v1(limb, Some(remaining_candidates))
    }
    fn derive_limb_inner_v1(
        &self,
        limb: usize,
        mut remaining_candidates: Option<&mut u64>,
    ) -> Result<Vec<u64>, ZkAmsMkheCpkRelationErrorV1> {
        if limb >= self.profile.moduli.len() {
            return Err(ZkAmsMkheCpkRelationErrorV1::NativeRelation);
        }
        let mut frame = [0_u8; active_collective_public_a_limb_frame_bytes_v1()];
        frame[..ACTIVE_COLLECTIVE_PUBLIC_A_PREFIX_BYTES_V1].copy_from_slice(&self.frame_prefix);
        frame[ACTIVE_COLLECTIVE_PUBLIC_A_PREFIX_BYTES_V1..].copy_from_slice(
            &u16::try_from(limb)
                .map_err(|_| ZkAmsMkheCpkRelationErrorV1::ResourceCeiling)?
                .to_be_bytes(),
        );
        let modulus = self.profile.moduli[limb];
        let zone = u64::MAX - u64::MAX % modulus;
        let mut stream = Shake256Reader::new(&frame);
        let mut coefficients = try_exact_capacity_vec_v1(self.profile.ring_degree)
            .map_err(|_| ZkAmsMkheCpkRelationErrorV1::ResourceCeiling)?;
        for _ in 0..self.profile.ring_degree {
            let mut accepted = None;
            for _ in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
                if let Some(remaining) = remaining_candidates.as_deref_mut() {
                    consume_common_a_candidate_budget_v1(remaining)?;
                }
                let mut bytes = [0_u8; 8];
                stream.read(&mut bytes);
                let candidate = u64::from_le_bytes(bytes);
                if candidate < zone {
                    accepted = Some(candidate % modulus);
                    break;
                }
            }
            coefficients.push(accepted.ok_or(ZkAmsMkheCpkRelationErrorV1::NativeRelation)?);
        }
        Ok(coefficients)
    }
}
fn validate_profile_digest_axis_v1(
    profile_digest: [u8; 32],
    roster_profile_digest: [u8; 32],
) -> Result<(), ZkAmsMkheCpkRelationErrorV1> {
    if profile_digest == [0; 32] || profile_digest != roster_profile_digest {
        return Err(ZkAmsMkheCpkRelationErrorV1::GovernedContext);
    }
    Ok(())
}
fn consume_common_a_candidate_budget_v1(
    remaining_candidates: &mut u64,
) -> Result<(), ZkAmsMkheCpkRelationErrorV1> {
    *remaining_candidates = (*remaining_candidates)
        .checked_sub(1)
        .ok_or(ZkAmsMkheCpkRelationErrorV1::ResourceCeiling)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn common_a_candidate_budget_accepts_boundary_and_rejects_one_over() {
        let mut remaining = 1_u64;
        assert_eq!(consume_common_a_candidate_budget_v1(&mut remaining), Ok(()));
        assert_eq!(remaining, 0);
        assert_eq!(
            consume_common_a_candidate_budget_v1(&mut remaining),
            Err(ZkAmsMkheCpkRelationErrorV1::ResourceCeiling)
        );
        assert_eq!(remaining, 0);
    }
    #[test]
    fn prepared_common_a_rejects_mismatched_profile_axis() {
        assert_eq!(
            validate_profile_digest_axis_v1([0x31; 32], [0x32; 32]),
            Err(ZkAmsMkheCpkRelationErrorV1::GovernedContext)
        );
        assert_eq!(
            validate_profile_digest_axis_v1([0; 32], [0; 32]),
            Err(ZkAmsMkheCpkRelationErrorV1::GovernedContext)
        );
    }
}
